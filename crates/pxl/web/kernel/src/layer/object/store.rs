// pxl-kernel/layer/object/store.rs

//! Object storage + spatial index. No "fine at current counts" here — this
//! is sized to work the same way at a handful of objects and at millions:
//! every mutation touches exactly the one L1 cell it affects (never a
//! whole-world walk), and every query is scoped to a cell or column (never a
//! scan over every placed object anywhere).
//!
//! The records are the second half: one per L2 column holding user objects —
//! a sparse octree per 64³ band whose leaves are catalog values — and,
//! derived from them a column at a time, the column's entries as three lanes
//! in an order a cell's are one run of. A coordinate is lowered here to a
//! column key, a band and a lattice position, and what comes back is lanes.
//! What this module adds is the eraser's two virtual edits, the revisions'
//! bumps, and the persisted form in the host's storage.

use std::collections::{BTreeMap, BTreeSet, HashMap, TryReserveError};

use base64::Engine;
use pxl_render::mesh::paint::object::adapter::Entries;
use pxl_spec::bake::constant::{
    band_cell_z, band_of, in_cell, pack_pos, unpack_pos, BandZ, CellZ, BAND, BAND_CELL_SHAMT, BAND_UNITS, CELL, OBJECT_FINEST, OBJECT_LAYOUT, POS_MASK, STEP_UNITS, U_PER_CELL,
};
use pxl_spec::bake::fold::majority;
use pxl_spec::bake::ground::shown_ground;
use pxl_spec::bake::pack::channel_of;
use pxl_spec::cell::object::{
    decor_id_from_num, decor_num_id, object_value_is_air, object_value_is_decor, object_value_is_voxel, placeable_cube_num_id, placeable_cube_of, terrain_style_cube_source, PlaceableCube,
};
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::hierarchy::{at_walk_order, child_of, first_descendant_of, parent_of, walk_order, Scale};
use pxl_spec::space::indexer::{FROM_L2_IDX, FROM_L3_IDX, L2_IDX, L3, L3_IDX};
use pxl_spec::space::pvf3::fvec::FVec;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pvf3::Ivec;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pyramid::{GROUND, TOP_LEVEL};
use pxl_spec::space::sparse_tree::mem;
use pxl_spec::space::sparse_tree::tree::{self as sparse_tree, SparseTree};
use pxl_spec::space::v3::{Aabb, V3};
use pxl_spec::space::xy::TO_XY;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::abi::host::{root, storage, STORAGE_KEYS};
use crate::global::Global;
use crate::host::{now, turn};
use crate::io::file::{self, Owner};
use crate::kernel::kernel;
use crate::layer::decor::build::{scatters_at, Draw};
use crate::shell::setting::store::hidden_ground;
use crate::world::access::{Access, FINEST};
use crate::world::change::{bump_world, wake_at, world_changes, world_edge, OBJECT_COLUMN};
use crate::world::chunk::source::{band_written, chunk_sources_changed};
use crate::world::frame::coord::local_world_to_canonical;
use xpute_core::status::bug::OrBug;

/// What a leaf holds, past its corner and size. A solid is the catalog's own
/// cube; the id it is spelled out under belongs to the two sides' shared
/// dictionary (spec/cell), not here.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Solid { cube: PlaceableCube },
    Decor { decor_id: DecorId },
    Air,
}

/// Where an entry is (pxl-spec bake/constant.rs): a holder of one reads the
/// entry back, hides it or erases it without a reverse lookup.
pub use pxl_spec::bake::constant::EntryAt;

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectLayerEntry {
    pub at: EntryAt,
    pub i: IVec,
    pub f: FVec,
    pub size: f64,
    pub kind: EntryKind,
}

/// Anything a placement/erase/selection query should actually treat as "a
/// real thing occupying this slot" — a solid color cube or a decor object
/// alike, just never `EntryKind::Air` (that's a virtual erase-marker, not a
/// placed thing). collision.rs, geometry.rs and camera/ray.rs only ever touch the
/// shared fields (i/f/size/at).
pub type OccupiedEntry = ObjectLayerEntry;

// A page is five bare lanes — x, y, z, size, value — with no record header
// (the persistence section below). A blob of an older encoding is not read:
// the key it is kept under carries the encoding's version (abi/host.json's
// STORAGE_KEYS), bumped whenever a change makes an older entry mean something
// other than it meant when it was written.

// ---- Column storage ----
//
// One record per L2 column that holds user objects, and inside it one
// sparse octree per 64³ band — the unified cell format of
// issue/20260911-decor-identity-orientation-and-one-cell-format.md. What a
// coordinate is keyed by — the column (the L2 index of its XY), the
// band's Z, and the position within the band in finest-tier steps — is `At`,
// each part a scale's index of the coordinate; this section also reads the
// lanes back.

/// The column a cell belongs to, as the key everything about objects is filed
/// under: the L2 index of the cell's XY column.
///
/// An index and not the column's address, so the key's three lanes are one
/// unit: the band's Z is an L2 index too, and a key whose XY was an address
/// would be a coordinate on two axes and a count on the third.
///
/// A column, so Z is dropped before the base is taken. The renderer asks for
/// a column by an XY page index and a page offset built with z = 0, so a key
/// that kept the cell's own Z put every object above absolute z = 64u (about
/// 120 m, one L2 span) under a key nothing ever asked for: stored, indexed,
/// hit-tested and never drawn.
fn column_of(cell: [i64; 3]) -> IVec {
    L2_IDX(TO_XY(CELL.base_of(cell).i))
}

/// The cell at a column's corner, on z = 0.
fn column_cell(col_idx: IVec) -> [i64; 3] {
    CELL.index_of(&Pvf3 { i: FROM_L2_IDX(col_idx), f: 0 })
}

/// Where a corner is filed: its column, its band, and its position within
/// the band in finest-tier steps. A corner finer than a step is not one an
/// entry can have, and lowering one drops the difference, as a write does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct At {
    pub col_idx: IVec,
    pub band: BandZ,
    pub step: [u32; 3],
}

impl At {
    /// Where a cube of `side` with its corner at `at` is filed: the corner
    /// lowered to the cube's own lattice, which is the only place a cube of
    /// that side can be, so a corner off it is not one to store another
    /// way than it is drawn, checked and placed.
    pub fn of_cube(at: &Pvf3, side: Scale) -> At {
        At::of(&side.base_of(side.index_of(at)))
    }

    pub fn of(at: &Pvf3) -> At {
        let cell = CELL.index_of(at);
        At {
            col_idx: column_of(cell),
            band: band_of(cell[2]).0,
            step: OBJECT_FINEST.index_of(at).map(|v| v as u32 & POS_MASK),
        }
    }

    /// The cell it is in.
    pub fn cell(self) -> [i64; 3] {
        let [x0, y0, _] = column_cell(self.col_idx);
        let ([cx, cy, cz], _) = in_cell(self.step);
        [x0 + i64::from(cx), y0 + i64::from(cy), band_cell_z(self.band) + i64::from(cz)]
    }

    /// The corner, exactly: the band's corner and the steps from it.
    pub fn corner(self) -> Pvf3 {
        let [x0, y0, _] = column_cell(self.col_idx);
        let mut out = Pvf3::default();
        pvf3::add(&mut out, &CELL.base_of([x0, y0, band_cell_z(self.band)]), &OBJECT_FINEST.base_of(self.step.map(i64::from)));
        out
    }

    /// The corner from the column's corner on z = 0, in units. Exact, since a
    /// step is a binary fraction of a unit.
    pub fn local(self) -> [f64; 3] {
        let [x, y, z] = self.step.map(|v| v as f64 * STEP_UNITS);
        [x, y, self.band as f64 * BAND_UNITS + z]
    }
}

/// The state stored at `coord`, VOID where nothing is.
fn read_state(coord: &Pvf3) -> u32 {
    let at = At::of(coord);
    kernel().objects.read(at.col_idx, at.band, at.step)
}

// ---- Entries ----
//
// What a column's trees hold, read back as one entry per leaf — a leaf larger
// than a cell split into the cells it covers, since everything downstream
// (collision's 3×3×3 neighborhood, a size never past one cell) assumes an
// entry is at most a cell. The kernel keeps them per column as three lanes —
// the band, a packed position, the state. A tree walked depth first visits a
// cell's leaves together, so a cell's entries are one contiguous run.
//
// `pos` packs the leaf's band-local position in finest-tier steps and its
// scale as the tiers above the finest (pxl-spec bake/constant.rs `pack_pos`).

/// One entry as a visit sees it. The column-local corner, size and state are
/// plain numbers; `corner()`, `cell()` and `at()` are worked out when asked
/// for, because few callers need them.
#[derive(Clone, Debug, Default)]
pub struct EntryCursor {
    /// Corner relative to the column's base (z = 0), in world units.
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
    /// The catalog value: a cube color, AIR, or a decor kind.
    pub state: u32,

    at: At,
    pos: u32,
}

impl EntryCursor {
    /// Points the cursor at a column; `load` then reads its entries.
    pub fn at_column(&mut self, col_idx: IVec) {
        self.at.col_idx = col_idx;
    }

    pub fn load(&mut self, band_idx: BandZ, pos: u32, state: u32) {
        let (x, y, z, scale) = unpack_pos(pos);
        self.at.band = band_idx;
        self.at.step = [x, y, z];
        self.pos = pos;
        [self.lx, self.ly, self.lz] = self.at.local();
        self.size = scale.side();
        self.state = state;
    }

    /// The cell the entry is in.
    pub fn cell(&self) -> [i64; 3] {
        self.at.cell()
    }

    /// The absolute corner.
    pub fn corner(&self) -> Pvf3 {
        self.at.corner()
    }

    /// Where the entry is filed.
    pub fn at(&self) -> EntryAt {
        EntryAt {
            col_idx: self.at.col_idx,
            band_idx: self.at.band,
            pos: self.pos,
        }
    }

    /// The entry as an object, for the few callers that keep one.
    pub fn object(&self) -> ObjectLayerEntry {
        let Pvf3 { i, f } = self.corner();
        let kind = if object_value_is_decor(self.state) {
            EntryKind::Decor {
                decor_id: decor_id_from_num(self.state),
            }
        } else if object_value_is_air(self.state) {
            EntryKind::Air
        } else {
            EntryKind::Solid {
                // Every value the store writes is one the catalog gives out.
                cube: placeable_cube_of(self.state).or_bug(Errno::ENOTRECOVERABLE),
            }
        };
        ObjectLayerEntry {
            at: self.at(),
            i,
            f,
            size: self.size,
            kind,
        }
    }
}

// ---- Cells ----
//
// A cell's entries are one run of its column's lanes, found where the cell
// falls in their order (Store::cell).

/// The run of the column's entries in the cell; None where the cell holds
/// nothing.
fn cell_run(ix: i64, iy: i64, iz: i64) -> Option<CellRun> {
    let col_idx = column_of([ix, iy, iz]);
    let [cx0, cy0, _] = column_cell(col_idx);
    kernel().objects.cell(col_idx, (ix - cx0) as u32, (iy - cy0) as u32, iz as i32)
}

/// Moves on every write to any column, so a reader that cached a derived
/// answer can tell whether the store has changed under it at all.
pub fn objects_revision() -> u32 {
    world_changes().tick(OBJECT_COLUMN)
}

/// Applies the writes to the store, which makes each changed column's
/// entries again inside the call, and announces every band changed.
fn apply(writes: impl IntoIterator<Item = Write>) {
    for (col_idx, band_idx) in kernel().objects.write(writes) {
        chunk_sources_changed();
        bump_world(OBJECT_COLUMN, col_idx);
        band_written(col_idx, band_idx);
    }
}

/// A cursor loaded at an address, for the two readers that hold one: the
/// cell it falls in, and the corner and size a write back to it needs.
fn cursor_at(at: EntryAt) -> EntryCursor {
    let mut cursor = EntryCursor::default();
    cursor.at_column(at.col_idx);
    cursor.load(at.band_idx, at.pos, 0);
    cursor
}

/// Every entry a column stores. The eraser's two edits are the drawn
/// build's alone (see set_virtual_hide), and none of these readers is one.
pub fn each_col_entry(col_idx: IVec, visit: &mut dyn FnMut(&EntryCursor)) {
    let Some(e) = kernel().objects.entries(col_idx) else { return };
    let mut cursor = EntryCursor::default();
    cursor.at_column(col_idx);
    for k in 0..e.band.len() {
        cursor.load(e.band[k], e.pos[k], e.state[k]);
        visit(&cursor);
    }
}

fn each_cell_entry(ix: i64, iy: i64, iz: i64, visit: &mut dyn FnMut(&EntryCursor)) {
    let Some(run) = cell_run(ix, iy, iz) else { return };
    let col_idx = column_of([ix, iy, iz]);
    let Some(e) = kernel().objects.entries(col_idx) else { return };
    let mut cursor = EntryCursor::default();
    cursor.at_column(col_idx);
    for k in run.start as usize..(run.start + run.count) as usize {
        cursor.load(e.band[k], e.pos[k], e.state[k]);
        visit(&cursor);
    }
}

fn occupants(ix: i64, iy: i64, iz: i64, out: &mut Vec<OccupiedEntry>) {
    each_cell_entry(ix, iy, iz, &mut |entry| {
        if !object_value_is_air(entry.state) {
            out.push(entry.object());
        }
    });
}

/// Every object claiming the cell — read live.
pub fn get_objects_in_cell(ix: i64, iy: i64, iz: i64) -> Vec<OccupiedEntry> {
    let mut out = Vec::new();
    occupants(ix, iy, iz, &mut out);
    out
}

/// Whether any leaf is written in the cell `at` lies in, air included: what
/// a scattered decor standing there yields to.
pub fn holds_entry(at: &Pvf3) -> bool {
    let [ix, iy, iz] = CELL.index_of(at);
    cell_run(ix, iy, iz).is_some()
}

/// Every object claiming the cell `at` stands in.
pub fn objects_at(at: &Pvf3) -> Vec<OccupiedEntry> {
    let [ix, iy, iz] = CELL.index_of(at);
    get_objects_in_cell(ix, iy, iz)
}

/// A cube as a node of the hierarchy: its side, and its index at that side,
/// the corner lowered to its own lattice as the store files it (`At`). Two
/// cubes share space exactly when one's node lies under the other's
/// (hierarchy.rs `share`), since every cube sits on its own size's lattice.
pub fn cube_node(corner: &Pvf3, size: f64) -> (Scale, [i64; 3]) {
    let side = Scale::of_side(size);
    (side, side.index_of(corner))
}

/// One object by its address, or None — the address says which cell it is
/// in, so this reads one cell's entries, not every object's.
pub fn get_placed_object(at: EntryAt) -> Option<OccupiedEntry> {
    let [ix, iy, iz] = cursor_at(at).cell();
    let mut found = None;
    each_cell_entry(ix, iy, iz, &mut |entry| {
        if found.is_none() && !object_value_is_air(entry.state) && entry.at() == at {
            found = Some(entry.object());
        }
    });
    found
}

/// The eraser's "dig into bare terrain" preview — see set_virtual_carve_preview.
#[derive(Clone, Copy, Debug)]
struct CarvePreview {
    cell: [i64; 3],
    at: EntryAt,
}

struct StoreState {
    /// The one object painted as absent from the real render build — see
    /// set_virtual_hide — and whether it is drawn in a chunk's tiers, its
    /// decor, or, where it is gone already, either.
    virtual_hide: Option<EntryAt>,
    hide_in: (bool, bool),
    virtual_carve_preview: Option<CarvePreview>,
    /// When the pending save is due, if one is.
    save_due_ms: Option<f64>,
    /// The saved blob the host is copying in, and the turn it was asked on:
    /// read on a later one.
    loading: Option<(Vec<u8>, u64)>,
}

static STORE: Global<StoreState> = Global::new();

fn store_state() -> &'static mut StoreState {
    STORE.get(|| StoreState {
        virtual_hide: None,
        hide_in: (false, false),
        virtual_carve_preview: None,
        save_due_ms: None,
        loading: None,
    })
}

/// Makes exactly one placed object paint as if it didn't exist, for the real
/// GPU-mesh build ONLY — the eraser's hover preview, which needs the real
/// target to stop rendering (so what is behind it shows) with a translucent
/// ghost of it on top.
///
/// Deliberately does NOT affect the cell queries (collision, decor's
/// occupancy) or the pick (world/access.rs): the object has not been removed,
/// and the pick is what resolves the hover in the first place — if it stopped
/// seeing the hidden object, hover would clear, the hide would lift, the
/// pick would hit it again: an infinite loop.
///
/// Nor is it a change to what is stored: no revision moves, so nothing that
/// reads the store — a save, the boxes a view measures heights by — sees
/// the hover. A chunk's tiers or a decor cell see it as one of their
/// sources (`render_edits`), by the entry's kind, so hover churn never
/// rebuilds the wrong one.
pub fn set_virtual_hide(at: Option<EntryAt>) {
    let st = store_state();
    if st.virtual_hide == at {
        return;
    }
    st.virtual_hide = at;
    let kind = at.and_then(get_placed_object).map(|o| o.kind);
    st.hide_in = (!matches!(kind, Some(EntryKind::Decor { .. })), !matches!(kind, Some(EntryKind::Solid { .. })));
    send_render_edits();
}

/// The eraser's "dig into bare terrain" preview — the terrain-side twin of
/// virtual_hide_id. There is no leaf to hide when the hover target is real
/// terrain, so this injects one AIR entry into the render build that was
/// never written — same render-only scoping as virtual_hide_id, for the same
/// reason. `i`/`f`/`size` match what a real place_air() at this hover would
/// write, so the preview renders identically to what it stands in for.
pub fn set_virtual_carve_preview(at: &Pvf3, size: f64) {
    let lowered = At::of_cube(at, Scale::of_side(size));
    let cell = lowered.cell();
    let st = store_state();
    if st.virtual_carve_preview.as_ref().is_some_and(|c| c.cell == cell) {
        return;
    }
    let [x, y, z] = lowered.step;
    let at = EntryAt {
        col_idx: lowered.col_idx,
        band_idx: lowered.band,
        pos: pack_pos(x, y, z, Scale::of_side(size)),
    };
    st.virtual_carve_preview = Some(CarvePreview { cell, at });
    send_render_edits();
}

pub fn clear_virtual_carve_preview() {
    if store_state().virtual_carve_preview.take().is_some() {
        send_render_edits();
    }
}

/// The drawn build's two edits, which a chunk's sources are compared with
/// beside the store (world/chunk/table.rs `Sources`), and a decor cell's
/// (layer/decor/cell.rs `Sources`): the hidden entry, with whether it is
/// drawn in tiers and in decor, and the carve preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderEdits {
    pub hide: Option<EntryAt>,
    pub hide_in: (bool, bool),
    pub carve: Option<EntryAt>,
}

pub fn render_edits() -> RenderEdits {
    let st = store_state();
    RenderEdits {
        hide: st.virtual_hide,
        hide_in: st.hide_in,
        carve: st.virtual_carve_preview.map(|c| c.at),
    }
}

/// Tells the builds the two edits, and the chunks that their sources moved.
fn send_render_edits() {
    let st = store_state();
    kernel().sched.builder.set_edits(st.virtual_hide, st.virtual_carve_preview.map(|c| c.at));
    chunk_sources_changed();
}

/// A column's revision: it moves on every write to what the column stores,
/// and on nothing else.
pub fn object_col_revision(col_idx: IVec) -> u32 {
    world_changes().revision_of(OBJECT_COLUMN, col_idx as i64)
}

// ---- Persistence ----

// A page's entries as five lanes, in a container that names the page:
//   [[page_idx u64, x f32[], y f32[], z f32[], size f32[], value u32[]], ...]
// Positions are page-local to FROM_L3_IDX(page_idx); a value is the catalog
// value the store writes (a block color, AIR, a decor kind). Nothing else is
// kept: a placed decor's facing is derived from its position
// (decor/build.rs's user_decor_pick), and an entry's address is rebuilt from
// where it lies. The layout is the code below and the version in the storage
// key; a blob from another layout is under another key and is never read.
//
// Positions round-trip exactly: the store's steps are binary fractions of a
// cell, and a page-local f32 holds any such value across a page.

/// The columns holding objects, in the kernel's order — for a walk over
/// everything placed, which is what a save is.
pub fn object_cols() -> Vec<IVec> {
    let objects = &kernel().objects;
    objects.cols().collect()
}

/// One page's entries, a lane a field.
#[derive(Default)]
struct PageRows {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    size: Vec<f32>,
    value: Vec<u32>,
}

pub fn encode_objects() -> Result<String, MarshalError> {
    let mut by_page: Vec<(IVec, PageRows)> = Vec::new();
    for col_idx in object_cols() {
        collect_col(&mut by_page, col_idx);
    }
    encode_pages(&by_page)
}

/// One column's entries added to its page's rows.
fn collect_col(by_page: &mut Vec<(IVec, PageRows)>, col_idx: IVec) {
    let page_idx = TO_XY(L3_IDX(FROM_L2_IDX(col_idx)));
    let page_base = FROM_L3_IDX(page_idx);
    let slot = match by_page.iter().position(|(p, _)| *p == page_idx) {
        Some(slot) => slot,
        None => {
            by_page.push((page_idx, PageRows::default()));
            by_page.len() - 1
        }
    };
    let rows = &mut by_page[slot].1;
    each_col_entry(col_idx, &mut |entry| {
        let corner = entry.corner();
        let local = Pvf3 {
            i: ivec::sub(corner.i, page_base),
            f: corner.f,
        };
        rows.x.push(pvf3::to_real(pvf3::X(&local)) as f32);
        rows.y.push(pvf3::to_real(pvf3::Y(&local)) as f32);
        rows.z.push(pvf3::to_real(pvf3::Z(&local)) as f32);
        rows.size.push(entry.size as f32);
        rows.value.push(entry.state);
    });
}

fn encode_pages(by_page: &[(IVec, PageRows)]) -> Result<String, MarshalError> {
    let mut tree = TreeView::new();
    tree.branch(Some(&mut |b| {
        for (page_idx, rows) in by_page {
            if rows.value.is_empty() {
                continue;
            }
            let value = &rows.value;
            b.branch(Some(&mut |p| {
                p.u64(Some(*page_idx))
                    .f32_array(Some(&rows.x))
                    .f32_array(Some(&rows.y))
                    .f32_array(Some(&rows.z))
                    .f32_array(Some(&rows.size))
                    .u32_array(Some(value));
            }));
        }
    }));
    let pkt = encoder().encode(
        &tree,
        TreeEncoderOptions {
            init_cap: Some(1 << 14),
            max_cap: None,
        },
    )?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&pkt))
}

pub fn decode_objects(blob: &str) -> Result<(), MarshalError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(blob).map_err(|_| MarshalError::new(Errno::EINVAL))?;
    let root = TreeReader::new(&bytes)?.read_branch()?;
    // Every record is one write of one batch, so each column's entries are
    // made once however many objects it holds.
    let mut writes = Vec::new();
    let result = (|| {
        for k in 0..root.len {
            let page = root.at_branch(k)?;
            let page_base = FROM_L3_IDX(page.at(0)?.get_u64()?);
            let (x, y, z) = (page.at(1)?.get_array::<f32>()?, page.at(2)?.get_array::<f32>()?, page.at(3)?.get_array::<f32>()?);
            let (size, value) = (page.at(4)?.get_array::<f32>()?, page.at(5)?.get_array::<u32>()?);
            for r in 0..value.len() {
                let local = [x[r] as f64, y[r] as f64, z[r] as f64];
                let mut corner = Pvf3::default();
                local_world_to_canonical(&mut corner, &local, page_base);
                writes.push(write_of(&corner, size[r] as f64, value[r]));
            }
        }
        Ok(())
    })();
    apply(writes);
    result
}

/// Opens the saved blob storage keeps, which the frame reads into the store
/// once it is in (`flush_save_if_due`).
pub fn load() {
    file::open(root::STORAGE, STORAGE_KEYS[storage::OBJECTS as usize], Owner::Objects);
}

/// The host's answer to the blob's open: read in, or none kept, which starts
/// the world empty.
pub fn objects_opened(fd: u32, res: i32) {
    if res < 0 {
        file::close(fd);
        return;
    }
    let mut blob = vec![0; res as usize];
    file::read(fd, &mut blob);
    file::close(fd);
    store_state().loading = Some((blob, turn()));
}

fn keep(blob: String) {
    file::write_whole(root::STORAGE, STORAGE_KEYS[storage::OBJECTS as usize], blob.into_bytes());
}

/// Reads a saved blob that has come in. An unreadable blob starts the world
/// empty, as a missing one does.
fn loaded() {
    let st = store_state();
    if !st.loading.as_ref().is_some_and(|(_, asked)| turn() > *asked) {
        return;
    }
    let (blob, _) = st.loading.take().or_bug(Errno::ENOTRECOVERABLE);
    if core::str::from_utf8(&blob).map_or(true, |raw| decode_objects(raw).is_err()) {
        kernel().objects.clear();
    }
}

/// The store is written whole — every entry walked and encoded — so a write
/// is not paid on the frame that placed something: the save is put off a
/// moment and a burst of writes (a drawn build's table, a quick run of
/// placements) is one save. What is pending is flushed when the page hides,
/// which is the last moment a tab on a phone is sure to get.
const SAVE_DELAY_MS: f64 = 300.0;

fn save() {
    let st = store_state();
    if st.save_due_ms.is_none() {
        st.save_due_ms = Some(now() + SAVE_DELAY_MS);
    }
}

static SAVES: Global<Notify> = Global::new();

fn saves() -> &'static mut Notify {
    SAVES.get(Notify::new)
}

/// The frame's check: a pending save that is due is asked of the save task,
/// and one not yet due asks for the turn it falls due on.
pub fn flush_save_if_due() {
    loaded();
    match store_state().save_due_ms {
        Some(due) if now() >= due => saves().notify(),
        Some(due) => wake_at(due),
        None => {}
    }
}

/// The save task: the store's columns walked on the turns' quota, pausing
/// between columns, then encoded and written in one step. A walk the store
/// moved under is not the store at any one moment, so it is dropped and the
/// save still pending is walked again. The save is taken as done only when
/// written, so the page hiding mid-walk still finds it (`flush_save`).
pub async fn save_task() {
    loop {
        notified(saves).await;
        if store_state().save_due_ms.is_none() {
            continue;
        }
        let revision = objects_revision();
        let mut by_page: Vec<(IVec, PageRows)> = Vec::new();
        let mut whole = true;
        for col_idx in object_cols() {
            yield_if_spent(world_edge).await;
            if objects_revision() != revision {
                whole = false;
                break;
            }
            collect_col(&mut by_page, col_idx);
        }
        yield_if_spent(world_edge).await;
        if !whole || objects_revision() != revision {
            saves().notify();
            continue;
        }
        if store_state().save_due_ms.take().is_none() {
            continue;
        }
        if let Ok(blob) = encode_pages(&by_page) {
            keep(blob);
        }
    }
}

/// Saves what is pending now: the page hiding.
pub fn flush_save() {
    if store_state().save_due_ms.take().is_none() {
        return;
    }
    if let Ok(blob) = encode_objects() {
        keep(blob);
    }
}

// ---- Mutations ----

/// i/f is what camera/ray.rs's world_adjacent_corner computed — the same
/// coordinate the hover preview and the overlap check used — filed on the
/// cube's own lattice (`At::of_cube`), which that corner already lies on, so
/// the write names the cube they did.
///
/// The kernel writes the leaf, compacts the band and drops it — and the
/// column — once every leaf in it reads back VOID: a redundant place() and a
/// remove() both write VOID on purpose, and a record carrying no information
/// would otherwise sit in every save for nothing.
fn write_object_value(coord: &Pvf3, size: f64, val: u32) {
    apply([write_of(coord, size, val)]);
    save();
}

/// The write that puts `val` at a corner, a leaf `size` on a side.
fn write_of(coord: &Pvf3, size: f64, val: u32) -> Write {
    // size is always an exact power of two by construction (object/size.rs's
    // OBJECT_SIZES); the nearest scale only guards against float noise.
    let scale = Scale::of_side(size);
    let at = At::of_cube(coord, scale);
    Write {
        col_idx: at.col_idx,
        band_idx: at.band,
        at: at.step,
        scale,
        state: val,
    }
}

/// Whether painting `cube` at this exact whole cell would actually change
/// anything a viewer could see — true only for a terrain-styled pick whose
/// own channel+type exactly matches what the REAL baked terrain already
/// shows there. Scoped to whole-cell writes only: a sub-cell pick paints a
/// fraction of the cell, never fully standing in for what the terrain layer
/// shows across the whole thing.
///
/// Two things have to hold, and each was wrong on its own.
///
/// The pick must match what the terrain actually *shows*, not merely a
/// channel it happens to carry — bake/ground.rs's shown_ground, the same
/// precedence the shader resolves a texture layer with. A road over bare
/// ground reads land=BARE and renders as road, so comparing the pick's own
/// channel alone canceled a bare-ground cube into a road.
///
/// And the cell must already hold an explicit AIR leaf, so the placement is
/// undoing a carve rather than adding a voxel terrain does not have. A ground
/// type is a per-column (2.5D) fact while a placement lands in the empty cell
/// above the surface, so without this every cube stacked on matching ground
/// read as redundant and was silently dropped.
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube, structures: bool) -> bool {
    if size < U_PER_CELL {
        return false;
    }
    let Some(source) = terrain_style_cube_source(cube) else { return false };
    // What the bake recorded is the leaf's to say: a page over it votes its
    // cells' types, which say nothing of this one. A placement acts on the
    // leaf, and where it is not resident the cube is written as asked.
    let Some(ground) = Access::now().at(at, FINEST).exact() else { return false };
    let shown = shown_ground(|ch| channel_of(ground.cell, ch) as u32, hidden_ground(structures));
    if shown.channel != source.channel || shown.type_id != source.terrain_num_id {
        return false;
    }
    object_value_is_air(read_state(at))
}

/// Carving a cell (place_air) and then re-painting it with the exact
/// terrain-styled cube the real ground already is there cancels back out to
/// nothing: rather than storing a "solid" leaf that only ever repaints what
/// terrain would already show, this writes VOID — the same absence remove()
/// itself produces.
pub fn place(at: &Pvf3, size: f64, cube: PlaceableCube, structures: bool) {
    let val = if place_value_is_redundant(at, size, cube, structures) {
        VOID
    } else {
        placeable_cube_num_id(cube)
    };
    write_object_value(at, size, val);
}

/// Same write path as place(), just tagged with a decor kind instead of a
/// color — and the same law: placing the decor the scatter itself stands in
/// a cell an erase emptied takes the erase back, VOID rather than a leaf
/// repeating what is made there, so the scattered instance returns as it
/// was, its own variant and turn.
pub fn place_decor(at: &Pvf3, size: f64, decor_id: DecorId) {
    let value = if restores_scattered(at, size, decor_id).is_some() { VOID } else { decor_num_id(decor_id) };
    write_object_value(at, size, value);
}

/// The scattered instance placing `decor_id` at `at` brings back, where it
/// is one an erase took away (`place_decor`): what the placement's preview
/// shows, since it is what the placement leaves.
pub fn restores_scattered(at: &Pvf3, size: f64, decor_id: DecorId) -> Option<Draw> {
    (size >= U_PER_CELL && object_value_is_air(read_state(at))).then(|| scatters_at(decor_num_id(decor_id), at))?
}

/// Eraser's own write path for its "nothing placed here" case — a real,
/// present leaf (cube_types::common::air) that means "nothing here, on
/// purpose," unlike remove() below, which deletes a leaf outright and lets
/// whatever's underneath (almost always real baked terrain) show through
/// again; that's no help against terrain itself, which has no object-layer
/// leaf of its own to delete.
pub fn place_air(at: &Pvf3, size: f64) {
    write_object_value(at, size, cube_types::common::air as u32);
}

pub fn remove(at: EntryAt) {
    // The address is all a write back to VOID needs: the cursor turns it into
    // the corner and the size, the same way a read of that entry does.
    let cursor = cursor_at(at);
    write_object_value(&cursor.corner(), cursor.size, VOID);
}

// ---- the records as the kernel holds them ----
//
// The object store's records; the paint that turns them into voxels is
// crates/pxl/web/render/src/mesh/paint/object/adapter.rs.
//
// One record per L2 column that holds user objects — a sparse octree per
// 64³ band whose leaves are catalog values — and, derived from it a column
// at a time, the column's entries as three lanes (band, packed position,
// state), in an order a cell's entries are one run of. Everything here is
// numbers: the client lowers a coordinate to a column key, a band and a
// lattice position before it crosses, and gets lanes back as views.
//
// A leaf is the catalog value itself, which the catalog keeps within a
// leaf's width (pxl-spec cell/object.rs); an entry is at most a cell because everything downstream — the collision neighborhood,
// the pick's cell by cell walk — assumes one, and a leaf covering more is
// split into the cells it covers as the lanes are built; and the lanes are
// made again a column at a time because a write touches one column and the
// cost is bounded by what that column holds.

/// The first of `0..n` for which `before` no longer holds, `before` holding
/// for a prefix of them.
fn partition_point(n: usize, before: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if before(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// A lattice position in a band, as its tree's key.
fn band_key(at: [u32; 3]) -> IVec {
    ivec::gather(at[0] as i64, at[1] as i64, at[2] as i64)
}

/// Whether every leaf of a band's tree reads VOID.
fn band_is_empty(tree: &SparseTree<u16>) -> bool {
    let mut empty = true;
    sparse_tree::walk(tree, &mut |_, _, _, _, idx| empty &= idx == 0);
    empty
}

/// The catalog's sentinels: the absence a zeroed leaf reads as, the eraser's
/// "nothing here, on purpose", and the value an entry never is.
pub const VOID: u32 = cube_types::common::void as u32;
pub const AIR: u32 = cube_types::common::air as u32;
pub const UNKNOWN: u32 = cube_types::common::unknown as u32;
/// A zeroed leaf has to read as the absence, because that is what a tree's
/// unwritten index is.
const _: () = assert!(VOID == 0);

pub(crate) struct Column {
    /// The band trees by Z, each leaf the catalog value it holds — VOID, a
    /// zeroed leaf, being the absence.
    bands: BTreeMap<BandZ, SparseTree<u16>>,
    /// The entries: the band trees walked in band order and each tree's walk
    /// order, a leaf coarser than a cell split into its cells in that order,
    /// so a cell's entries are one run and the lanes are sorted by
    /// (band, cell's walk order). Made again whenever a write changes the
    /// column, inside the write.
    pub(crate) e_band: Vec<BandZ>,
    pub(crate) e_pos: Vec<u32>,
    pub(crate) e_state: Vec<u32>,
    /// Each band's entries' box, in units from the column's corner across
    /// and from zero up: made with the entries.
    bounds: BTreeMap<BandZ, Aabb>,
}

impl Column {
    /// Re-reads the trees into the entry lanes: one entry per leaf, a leaf
    /// larger than a cell split into the cells it covers.
    fn derive_entries(&mut self) {
        self.e_band.clear();
        self.e_pos.clear();
        self.e_state.clear();
        self.bounds.clear();
        for (&bz, tree) in &self.bands {
            let mut bound: Option<Aabb> = None;
            sparse_tree::walk(tree, &mut |x, y, z, scale, value| {
                let (x, y, z, val) = (x as u32, y as u32, z as u32, u32::from(value));
                if val == VOID || val == UNKNOWN {
                    return;
                }
                let min = [x as f64 * STEP_UNITS, y as f64 * STEP_UNITS, bz as f64 * BAND_UNITS + z as f64 * STEP_UNITS];
                let b = Aabb {
                    min,
                    max: min.map(|v| v + scale.side()),
                };
                bound = Some(bound.map_or(b, |a| a.join(b)));
                if scale <= CELL {
                    self.e_band.push(bz);
                    self.e_pos.push(pack_pos(x, y, z, scale));
                    self.e_state.push(val);
                    return;
                }
                // Its cells in the walk's own order, so the lanes stay
                // sorted by it.
                let levels = scale.above(CELL) as u32;
                for k in 0..1u64 << (3 * levels) {
                    let cell = at_walk_order::<3>(k, levels).map(i64::from);
                    let [dx, dy, dz] = first_descendant_of(cell, CELL.above(OBJECT_FINEST) as u32).map(|v| v as u32);
                    self.e_band.push(bz);
                    self.e_pos.push(pack_pos(x + dx, y + dy, z + dz, CELL));
                    self.e_state.push(val);
                }
            });
            if let Some(b) = bound {
                self.bounds.insert(bz, b);
            }
        }
    }
}

/// The run [start, start + count) of a column's entries in one cell, and how
/// many of them really occupy it — every kind but AIR.
#[derive(Clone, Copy, Debug, Default)]
pub struct CellRun {
    pub start: u32,
    pub count: u32,
    pub solid: u32,
}

/// One write: a leaf of `scale` holding `state` at a lattice position of band
/// `band_idx` of the column `col_idx`.
#[derive(Clone, Copy, Debug)]
pub struct Write {
    pub col_idx: IVec,
    pub band_idx: BandZ,
    pub at: [u32; 3],
    pub scale: Scale,
    pub state: u32,
}

pub struct Store {
    /// A record for every column holding an object, and only for those: a
    /// band is dropped once every leaf of it reads VOID, and a column with
    /// its last band.
    columns: BTreeMap<IVec, Column>,
    /// Where the objects are: for every cube from a band's up to `INDEX_TOP`
    /// that holds any entry — a solid, a decor or air — the box they fill,
    /// in units, by the cube's scale and its index at it. A band's is its
    /// entries' and a cube's the union of its eight children's, so a write
    /// makes again the one chain over the band it changed, and a cube no
    /// entry is in has none. No axis is kept apart: what a box holds and how
    /// high it reaches are both read off the one bound (`bound_in`).
    bounds: HashMap<(Scale, [i64; 3]), Aabb>,
    /// What the objects fold to at every cell from a band's up to the top
    /// level's, by the cell's scale and its index at it, none where that is
    /// nothing: a band's is its tree's root block folded, and each cell over
    /// it its eight children's (`fold_cell`). Under a band the band's own
    /// tree keeps it, a branch's in its slot (`set_folded`). So what the
    /// objects look like at any cell is one read, whatever lies under it.
    folds: HashMap<(Scale, [i64; 3]), u16>,
}

/// What eight children fold to, a level up: the object layer's cell from
/// the 2×2×2 under it, by the rule the terrain folds by one dimension down
/// (pxl-spec bake/fold.rs `majority`) — the value that fills at least half
/// of eight, empty counted as a value, so a lone cube goes at the first
/// fold and a wall keeps whole cells — a value that paints no voxel counted
/// as empty, the tie by `scale`'s level. An entry is a leaf of its band's
/// tree, and a leaf a block uniform all the way down, so a tree folded a
/// level at a time is every entry folded from the finest up, each joining
/// at its own size.
fn fold_cell(children: &[u16], scale: Scale) -> u16 {
    let v: [u16; 8] = core::array::from_fn(|k| if object_value_is_voxel(u32::from(children[k])) { children[k] } else { VOID as u16 });
    majority(v, scale.above(CELL))
}

/// The scale of the top level's cell: the coarsest the objects are folded
/// to.
const TOP_CELL: Scale = GROUND.scale(TOP_LEVEL);

/// The coarsest cube the index keeps: the first from a band's up as wide as
/// the widest box anything asks of it on any axis — as tall as a position can
/// reach (pxl-spec space/pvf3.rs `REACH`), and as wide as the coarsest page,
/// which bounds every box the kernel asks across: a cell or a page of a
/// level, a chunk with the apron of one cell a fold reads, a node of the
/// ground's index. A box no longer than a cube along an axis meets at most
/// two of them along it, so every query starts from at most eight, and what
/// it costs is the cubes under those it crosses, not the size it spans. One
/// band's entries add a cube at each scale up to it and no more: the index
/// holds at most its depth over a band for each band.
const INDEX_TOP: Scale = {
    let tall = REACH.max[2] - REACH.min[2];
    let wide = L3.up(TOP_LEVEL as i32).side();
    let mut s = BAND;
    while s.side() < tall || s.side() < wide {
        s = s.up(1);
    }
    s
};

/// Whether `inner` lies wholly within `outer`.
fn within(inner: &Aabb, outer: &Aabb) -> bool {
    (0..3).all(|k| inner.min[k] >= outer.min[k] && inner.max[k] <= outer.max[k])
}

/// Makes again the chain of cubes in `map` over the one of `scale` at `at`,
/// up to `top`: that one `first`, and each over it what `reduce` makes of
/// its eight children at the scale it is, a cube that comes to nothing
/// taken out. What the objects fold to and where they are both change
/// only along this one path when a band does.
fn propagate_up<V: Copy>(map: &mut HashMap<(Scale, [i64; 3]), V>, (mut scale, mut at): (Scale, [i64; 3]), top: Scale, first: Option<V>, reduce: impl Fn(Scale, [Option<V>; 8]) -> Option<V>) {
    let mut v = first;
    loop {
        match v {
            Some(v) => map.insert((scale, at), v),
            None => map.remove(&(scale, at)),
        };
        if scale >= top {
            return;
        }
        (at, scale) = (parent_of(at), scale.up(1));
        v = reduce(scale, core::array::from_fn(|k| map.get(&(scale.down(1), child_of(at, k))).copied()));
    }
}

/// `b` grown out to the lattice of cells `side` wide: every cell it
/// touches, whole.
fn grown(b: &Aabb, side: f64) -> Aabb {
    Aabb {
        min: b.min.map(|v| (v / side).floor() * side),
        max: b.max.map(|v| (v / side).ceil() * side),
    }
}

/// Where along the ray `(origin, direction)` it is inside `b` within `span`;
/// none where it is not.
fn slab((o, d): (V3, V3), b: &Aabb, (t0, t1): (f64, f64)) -> Option<(f64, f64)> {
    let (enter, leave, _) = b.span_along(o, d)?;
    let (enter, leave) = (enter.max(t0), leave.min(t1));
    (enter <= leave).then_some((enter, leave))
}

/// The cells `side` wide the ray `(origin, direction)` passes between `t0`
/// and `t1`, in order, each with where along the ray it is in the cell,
/// until `each` says to stop: whether it did (Amanatides–Woo).
fn cells_along((o, d): (V3, V3), (t0, t1): (f64, f64), side: f64, each: &mut dyn FnMut([i64; 3], (f64, f64)) -> bool) -> bool {
    let start = [0, 1, 2].map(|k| o[k] + d[k] * t0);
    let mut cell = start.map(|v| (v / side).floor() as i64);
    let mut next = [0.0; 3];
    let mut delta = [0.0; 3];
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            (next[k], delta[k]) = (f64::INFINITY, f64::INFINITY);
        } else {
            let face = (cell[k] + i64::from(d[k] > 0.0)) as f64 * side;
            (next[k], delta[k]) = ((face - o[k]) / d[k], side / d[k].abs());
        }
    }
    let mut from = t0;
    loop {
        let k = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b])).unwrap_or(0);
        let to = next[k].min(t1);
        if each(cell, (from, to)) {
            return true;
        }
        if next[k] >= t1 {
            return false;
        }
        cell[k] += if d[k] > 0.0 { 1 } else { -1 };
        from = next[k];
        next[k] += delta[k];
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Store {
        Store {
            columns: BTreeMap::new(),
            bounds: HashMap::new(),
            folds: HashMap::new(),
        }
    }

    /// The columns holding objects.
    pub fn cols(&self) -> impl Iterator<Item = IVec> + '_ {
        self.columns.keys().copied()
    }

    /// Where the objects in `query` are: a box holding every part of every
    /// entry inside it, within it, or none where there is none — never none
    /// where there is some. Down the index from the cubes over `query`, a
    /// cube whose bound misses it is passed, one wholly inside gives its
    /// bound, and one across its edge is looked into; a band's, the finest
    /// kept, gives its bound cut to `query`, which may hold air between its
    /// entries, so a box can be given where no entry is.
    pub fn bound_in(&self, query: &Aabb) -> Option<Aabb> {
        let mut out: Option<Aabb> = None;
        self.descend(query, BAND, &mut |scale, at, bound| {
            let whole = within(&scale.cube(at), query) || scale == BAND;
            if let Some(b) = bound.meet(*query).filter(|_| whole) {
                out = Some(out.map_or(b, |o| o.join(b)));
            }
            !whole
        });
        out
    }

    /// Every cube of `scale`, a band's or coarser, holding an entry whose
    /// bound meets `query`, and its bound: what a reader walks cell by cell,
    /// found down the same index as `bound_in`.
    pub fn visit_in(&self, query: &Aabb, scale: Scale, each: &mut dyn FnMut([i64; 3], &Aabb)) {
        self.descend(query, scale, &mut |s, at, bound| {
            if s == scale {
                each(at, bound);
            }
            s != scale
        });
    }

    /// Down the index from every `INDEX_TOP` cube `query` meets, to cubes of
    /// `finest`: each cube kept whose bound meets `query` is handed to
    /// `enter`, and its children are looked into where it returns true and
    /// it is coarser than `finest`.
    fn descend(&self, query: &Aabb, finest: Scale, enter: &mut dyn FnMut(Scale, [i64; 3], &Aabb) -> bool) {
        fn go(store: &Store, query: &Aabb, finest: Scale, scale: Scale, at: [i64; 3], enter: &mut dyn FnMut(Scale, [i64; 3], &Aabb) -> bool) {
            let Some(bound) = store.bounds.get(&(scale, at)) else { return };
            if bound.meet(*query).is_none() || !enter(scale, at, bound) || scale <= finest {
                return;
            }
            for k in 0..8 {
                go(store, query, finest, scale.down(1), child_of(at, k), enter);
            }
        }
        let side = INDEX_TOP.side();
        let range = |k: usize| (query.min[k].max(REACH.min[k]) / side).floor() as i64..=(query.max[k].min(REACH.max[k]) / side).floor() as i64;
        for z in range(2) {
            for y in range(1) {
                for x in range(0) {
                    go(self, query, finest, INDEX_TOP, [x, y, z], enter);
                }
            }
        }
    }

    /// The columns holding objects under a node, a column's or coarser, at
    /// every height, added to `out`, sorted: found down the index, so the
    /// cost is what is under the node and not what the store holds.
    pub fn cols_under(&self, (side, at): (Scale, [i64; 2]), out: &mut Vec<IVec>) -> Result<(), TryReserveError> {
        let first = out.len();
        let s = side.side();
        let query = Aabb {
            min: [at[0] as f64 * s, at[1] as f64 * s, REACH.min[2]],
            max: [(at[0] + 1) as f64 * s, (at[1] + 1) as f64 * s, REACH.max[2]],
        };
        let mut refused = Ok(());
        self.visit_in(&query, BAND, &mut |[x, y, _], _| {
            if refused.is_ok() {
                refused = try_push(out, ivec::gather(x, y, 0));
            }
        });
        refused?;
        out[first..].sort_unstable();
        let mut n = first;
        for k in first..out.len() {
            if k == first || out[k] != out[n - 1] {
                out[n] = out[k];
                n += 1;
            }
        }
        out.truncate(n);
        Ok(())
    }

    /// Every cell of `scale` that may hold objects along the ray from
    /// `origin` along `direction` between `t0` and `t1`, in the order the
    /// ray passes them, until `each` says to stop: what a ray meets of the
    /// objects is found down the index along the ray itself. From each
    /// `INDEX_TOP` cube it crosses, in turn, a cube is looked into only where
    /// the ray passes through its bound grown out to whole cells of `scale`
    /// — what the cells drawn there fill — and its children in the order the
    /// ray enters them, which is the order it passes them, since they do not
    /// overlap. Over a band, a cell is a cube the index keeps; under it,
    /// every cell the ray passes inside the band's grown bound. Whether a
    /// cell holds anything the caller reads is the caller's.
    pub fn along_ray(&self, origin: V3, direction: V3, (t0, t1): (f64, f64), scale: Scale, each: &mut dyn FnMut([i64; 3]) -> bool) {
        let ray = (origin, direction);
        cells_along(ray, (t0, t1), INDEX_TOP.side(), &mut |at, span| self.along_ray_from(ray, span, scale, INDEX_TOP, at, each));
    }

    /// `along_ray` from the cube of `node` at `at`, the ray over `span`: whether
    /// `each` said to stop.
    fn along_ray_from(&self, ray: (V3, V3), span: (f64, f64), scale: Scale, node: Scale, at: [i64; 3], each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
        let Some(bound) = self.bounds.get(&(node, at)) else { return false };
        let Some(span) = slab(ray, &grown(bound, scale.side()), span) else { return false };
        if node == scale {
            return each(at);
        }
        if node == BAND && scale < BAND {
            return cells_along(ray, span, scale.side(), &mut |cell, _| each(cell));
        }
        let mut kids = [(0.0, [0i64; 3], (0.0, 0.0)); 8];
        let mut n = 0;
        for k in 0..8 {
            let child = child_of(at, k);
            let Some(b) = self.bounds.get(&(node.down(1), child)) else { continue };
            if let Some(s) = slab(ray, &grown(b, scale.side()), span) {
                kids[n] = (s.0, child, s);
                n += 1;
            }
        }
        kids[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        kids[..n].iter().any(|&(_, child, s)| self.along_ray_from(ray, s, scale, node.down(1), child, each))
    }

    /// Makes again where the objects are over band `band_idx` of column
    /// `col_idx`, from the band up to `INDEX_TOP`.
    fn index_up(&mut self, col_idx: IVec, band_idx: BandZ) {
        let at = [ivec::X(col_idx), ivec::Y(col_idx), i64::from(band_idx)];
        let corner = FROM_L2_IDX(col_idx);
        let (x0, y0) = (ivec::X(corner) as f64, ivec::Y(corner) as f64);
        let bound = self.columns.get(&col_idx).and_then(|c| c.bounds.get(&band_idx)).map(|b| Aabb {
            min: [b.min[0] + x0, b.min[1] + y0, b.min[2]],
            max: [b.max[0] + x0, b.max[1] + y0, b.max[2]],
        });
        propagate_up(&mut self.bounds, (BAND, at), INDEX_TOP, bound, |_, children| children.into_iter().flatten().reduce(Aabb::join));
    }

    /// The state at a lattice position of band `band_idx` of column
    /// `col_idx`; VOID where nothing is.
    pub fn read(&self, col_idx: IVec, band_idx: BandZ, at: [u32; 3]) -> u32 {
        let Some(tree) = self.columns.get(&col_idx).and_then(|c| c.bands.get(&band_idx)) else {
            return VOID;
        };
        u32::from(mem::get::<Ivec, u16>(&tree.mem, &tree.layout, band_key(at), 0))
    }

    /// Applies every write, then makes again the entries of each column one
    /// changed, once: what a reader sees is the trees, whatever it reads.
    /// The bands changed, each with its column, for the caller to announce:
    /// a band is a z16 chunk's height, so a column and a band are the chunk
    /// a write is in (world/pyramid/chunk.rs).
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> BTreeSet<(IVec, BandZ)> {
        let mut changed = BTreeSet::new();
        for w in writes {
            if self.put(w) {
                changed.insert((w.col_idx, w.band_idx));
            }
        }
        // Each band a write changed is compacted once, and dropped — and its
        // column — once every leaf of it reads VOID: after the writes, so a
        // write costs its path and a batch a walk of each band it changed.
        for &(col_idx, band_idx) in &changed {
            let Some(column) = self.columns.get_mut(&col_idx) else { continue };
            let Some(tree) = column.bands.get_mut(&band_idx) else { continue };
            sparse_tree::compact(tree);
            if band_is_empty(tree) {
                column.bands.remove(&band_idx);
                if column.bands.is_empty() {
                    self.columns.remove(&col_idx);
                }
            }
        }
        let columns: BTreeSet<IVec> = changed.iter().map(|&(col_idx, _)| col_idx).collect();
        for &col_idx in &columns {
            if let Some(column) = self.columns.get_mut(&col_idx) {
                column.derive_entries();
            }
        }
        for &(col_idx, band_idx) in &changed {
            self.fold_up(col_idx, band_idx);
            self.index_up(col_idx, band_idx);
        }
        changed
    }

    /// The objects under the cells `from` to `to` of `target` on the ground,
    /// at every height, as the objects' fold makes them (`fold_cell`): each
    /// cell there and what it folds to, none that folds to nothing, into
    /// `out`. Read at the cell from what
    /// the store keeps folded, so what it costs is the cells there and not
    /// what lies under them. `hide`, the one entry the eraser shows gone, is
    /// left out as the fold leaves it out.
    pub fn fold_at(&self, target: Scale, from: [i64; 2], to: [i64; 2], hide: Option<EntryAt>, out: &mut HashMap<[i64; 3], u32>) -> Result<(), TryReserveError> {
        out.clear();
        let mut refused = Ok(());
        let inside = |x: i64, y: i64| (from[0]..to[0]).contains(&x) && (from[1]..to[1]).contains(&y);
        if target >= BAND {
            let side = target.side();
            let query = Aabb {
                min: [from[0] as f64 * side, from[1] as f64 * side, REACH.min[2]],
                max: [to[0] as f64 * side, to[1] as f64 * side, REACH.max[2]],
            };
            self.visit_in(&query, target, &mut |at, _| {
                if let Some(&v) = self.folds.get(&(target, at)).filter(|_| inside(at[0], at[1])) {
                    if refused.is_ok() {
                        refused = try_insert(out, at, u32::from(v));
                    }
                }
            });
        } else {
            // In cells of `target`: a column's side and a band's height, and
            // a lattice step's.
            let (cells, per) = (target.count_in(BAND) as i64, OBJECT_FINEST.count_in(target) as i64);
            for cy in from[1].div_euclid(cells)..=(to[1] - 1).div_euclid(cells) {
                for cx in from[0].div_euclid(cells)..=(to[0] - 1).div_euclid(cells) {
                    let Some(column) = self.columns.get(&ivec::gather(cx, cy, 0)) else { continue };
                    for (&band_idx, tree) in &column.bands {
                        let corner = [cx * cells, cy * cells, i64::from(band_idx) * cells];
                        mem::walk_to(
                            &tree.mem,
                            &tree.layout,
                            target,
                            &mut |x, y, z, s, v| {
                                if !object_value_is_voxel(u32::from(v)) {
                                    return;
                                }
                                // A leaf coarser than the cell stands in every cell under it.
                                let n = target.count_in(s) as i64;
                                let at = [x, y, z].map(|c| i64::from(c) / per);
                                for dz in 0..n {
                                    for dy in 0..n {
                                        for dx in 0..n {
                                            let cell = [corner[0] + at[0] + dx, corner[1] + at[1] + dy, corner[2] + at[2] + dz];
                                            if inside(cell[0], cell[1]) && refused.is_ok() {
                                                refused = try_insert(out, cell, u32::from(v));
                                            }
                                        }
                                    }
                                }
                            },
                            0,
                        );
                    }
                }
            }
        }
        refused?;
        if let Some((cell, v)) = hide.and_then(|h| self.hidden(h, target)) {
            if inside(cell[0], cell[1]) {
                match v {
                    0 => {
                        out.remove(&cell);
                    }
                    v => try_insert(out, cell, u32::from(v))?,
                }
            }
        }
        Ok(())
    }

    /// Whether anything is kept in the band cell `at` (a `CELL`) lies in: false
    /// says nothing is placed there, true only that something may be.
    pub fn may_hold(&self, at: [i64; 3]) -> bool {
        let cells = CELL.count_in(BAND) as i64;
        let col_idx = ivec::gather(at[0].div_euclid(cells), at[1].div_euclid(cells), 0);
        self.columns.get(&col_idx).is_some_and(|c| c.bands.contains_key(&(at[2].div_euclid(cells) as BandZ)))
    }

    /// The cell over the highest band anything is kept in, in the column of
    /// cell `[x, y]` (a `CELL`): no cell at it or above holds an object.
    pub fn ceiling(&self, [x, y]: [i64; 2]) -> Option<i64> {
        let cells = CELL.count_in(BAND) as i64;
        let col_idx = ivec::gather(x.div_euclid(cells), y.div_euclid(cells), 0);
        let band = *self.columns.get(&col_idx)?.bands.keys().next_back()?;
        Some((i64::from(band) + 1) * cells)
    }

    /// What the objects fold to at cell `at` of `scale` (`fold_cell`), VOID
    /// where nothing: one read, wherever it is kept — a value that paints no
    /// voxel as it is, for the reader to count as nothing.
    pub fn folded_value(&self, scale: Scale, at: [i64; 3]) -> u16 {
        if scale >= BAND {
            return self.folds.get(&(scale, at)).copied().unwrap_or(VOID as u16);
        }
        let cells = scale.count_in(BAND) as i64;
        let col_idx = ivec::gather(at[0].div_euclid(cells), at[1].div_euclid(cells), 0);
        let Some(tree) = self.columns.get(&col_idx).and_then(|c| c.bands.get(&(at[2].div_euclid(cells) as BandZ))) else {
            return VOID as u16;
        };
        let step = OBJECT_FINEST.count_in(scale) as i64;
        let key = band_key(at.map(|c| (c.rem_euclid(cells) * step) as u32));
        tree.mem.data[mem::resolve::<Ivec, u16>(&tree.mem, &tree.layout, key, scale, 0) as usize]
    }

    /// The cell of `target` the entry `hide` is in, and what it folds to with
    /// the entry gone: the fold made again up the entry's one path, its
    /// siblings read where they are kept. None where nothing is there.
    /// Column `col_idx`'s voxel entries as a paint at `target` takes them:
    /// a leaf finer than `target` folded to it (`fold_cell`), and one at
    /// least as coarse kept whole, at its own size. `fold_at` answers which
    /// cells are filled, so a coarse leaf is in every cell under it there;
    /// painted cell by cell, a placed 1u cube would come out as a quarter
    /// cell's sixty-four sub-cell entries, which the mesher draws a box each
    /// and never merges — one block seen split 4 × 4. `visit` is handed the
    /// band, the entry's corner in the band's finest steps, its scale and its
    /// state; `hide`, the one entry the eraser shows gone, is left out as the
    /// fold leaves it out.
    pub fn fold_entries(&self, col_idx: IVec, target: Scale, hide: Option<EntryAt>, visit: &mut dyn FnMut(BandZ, [u32; 3], Scale, u32)) {
        let Some(column) = self.columns.get(&col_idx) else { return };
        let hidden = hide.filter(|h| h.col_idx == col_idx).and_then(|h| {
            let own = unpack_pos(h.pos).3;
            let (at, v) = self.hidden(h, target)?;
            Some((h.band_idx, at, if own > target { own } else { target }, u32::from(v)))
        });
        for (&band_idx, tree) in &column.bands {
            let corner = [ivec::X(col_idx), ivec::Y(col_idx), i64::from(band_idx)];
            mem::walk_to(
                &tree.mem,
                &tree.layout,
                target,
                &mut |x, y, z, s, v| {
                    let mut v = u32::from(v);
                    if let Some((band, at, scale, w)) = hidden {
                        let (cells, per) = (s.count_in(BAND) as i64, OBJECT_FINEST.count_in(s) as i64);
                        let cell: [i64; 3] = core::array::from_fn(|k| corner[k] * cells + i64::from([x, y, z][k]) / per);
                        if band == band_idx && scale == s && cell == at {
                            v = w;
                        }
                    }
                    if object_value_is_voxel(v) {
                        visit(band_idx, [x, y, z].map(|c| c as u32), s, v);
                    }
                },
                0,
            );
        }
    }

    fn hidden(&self, hide: EntryAt, target: Scale) -> Option<([i64; 3], u16)> {
        self.columns.get(&hide.col_idx)?.bands.get(&hide.band_idx)?;
        let (x, y, z, own) = unpack_pos(hide.pos);
        let per = OBJECT_FINEST.count_in(own) as i64;
        let cells = own.count_in(BAND) as i64;
        let corner = [ivec::X(hide.col_idx), ivec::Y(hide.col_idx), i64::from(hide.band_idx)].map(|c| c * cells);
        let mut at: [i64; 3] = core::array::from_fn(|k| corner[k] + i64::from([x, y, z][k]) / per);
        let (mut scale, mut v) = (own, VOID as u16);
        while scale < target {
            let (parent, up) = (parent_of(at), scale.up(1));
            let children: [u16; 8] = core::array::from_fn(|k| {
                let c = child_of(parent, k);
                if c == at {
                    v
                } else {
                    self.folded_value(scale, c)
                }
            });
            (at, scale, v) = (parent, up, fold_cell(&children, up));
        }
        Some((at, v))
    }

    /// Makes again what the objects fold to over band `band_idx` of column
    /// `col_idx`, from the band up to the top level's cell.
    fn fold_up(&mut self, col_idx: IVec, band_idx: BandZ) {
        let at = [ivec::X(col_idx), ivec::Y(col_idx), i64::from(band_idx)];
        let band = self.columns.get(&col_idx).and_then(|c| c.bands.get(&band_idx));
        let v = band.map(|t| fold_cell(&t.mem.data[..8], BAND)).filter(|&v| v != VOID as u16);
        propagate_up(&mut self.folds, (BAND, at), TOP_CELL, v, |scale, children| {
            let v = fold_cell(&children.map(|c| c.unwrap_or(VOID as u16)), scale);
            (v != VOID as u16).then_some(v)
        });
    }

    /// Writes one leaf, the folds over it kept (`fold_cell`). False where
    /// nothing was there to change: VOID written where no band is.
    fn put(&mut self, w: Write) -> bool {
        let Write { col_idx, band_idx, at, scale, state } = w;
        xpute_core::ensure!((OBJECT_FINEST..=CELL).contains(&scale), ENOTRECOVERABLE);
        let value = u16::try_from(state).or_bug(Errno::EOVERFLOW);
        let has_band = self.columns.get(&col_idx).is_some_and(|c| c.bands.contains_key(&band_idx));
        if !has_band && state == VOID {
            return false;
        }
        let column = self.columns.entry(col_idx).or_insert_with(|| Column {
            bands: BTreeMap::new(),
            e_band: Vec::new(),
            e_pos: Vec::new(),
            e_state: Vec::new(),
            bounds: BTreeMap::new(),
        });
        let tree = column.bands.entry(band_idx).or_insert_with(|| sparse_tree::alloc::<u16>(OBJECT_LAYOUT, 64));
        mem::set_folded::<Ivec, u16>(&mut tree.mem, &tree.layout, band_key(at), value, scale, 0, &fold_cell);
        true
    }

    /// Where an entry's cell falls in the lanes' order: its band, then the
    /// cell's place in its band's walk.
    fn cell_order(band_idx: BandZ, cx: u32, cy: u32, cz: u32) -> (BandZ, u64) {
        (band_idx, walk_order([cx, cy, cz], BAND_CELL_SHAMT))
    }

    /// A column's entries as the view paints them, or none where the column
    /// holds nothing: a column is held only while a leaf of it is not VOID,
    /// and every such leaf is an entry.
    pub fn entries(&self, col_idx: IVec) -> Option<Entries<'_>> {
        let column = self.columns.get(&col_idx)?;
        Some(Entries {
            col_idx,
            band: &column.e_band,
            pos: &column.e_pos,
            state: &column.e_state,
        })
    }

    /// The run of a column's entries in the cell at band-local (cx, cy) and
    /// absolute cell Z `cz`: found where it lies in the lanes' order.
    pub fn cell(&self, col_idx: IVec, cx: u32, cy: u32, cz: CellZ) -> Option<CellRun> {
        let column = self.columns.get(&col_idx)?;
        let (band_idx, z) = band_of(cz as i64);
        let want = Self::cell_order(band_idx, cx, cy, z);
        let order = |k: usize| {
            let (x, y, z, _) = unpack_pos(column.e_pos[k]);
            let ([x, y, z], _) = in_cell([x, y, z]);
            Self::cell_order(column.e_band[k], x, y, z)
        };
        let n = column.e_band.len();
        let start = partition_point(n, |k| order(k) < want);
        let end = start + partition_point(n - start, |k| order(start + k) == want);
        (end > start).then(|| CellRun {
            start: start as u32,
            count: (end - start) as u32,
            solid: (start..end).filter(|&k| column.e_state[k] != AIR).count() as u32,
        })
    }

    /// Every record gone, as a load that could not be read leaves the world.
    pub fn clear(&mut self) {
        self.columns.clear();
        self.bounds.clear();
        self.folds.clear();
    }
}

/// `v` pushed where the heap has room for it: the vector grows by doubling,
/// asked for only when full.
fn try_push<T>(out: &mut Vec<T>, v: T) -> Result<(), TryReserveError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))?;
    }
    out.push(v);
    Ok(())
}

/// `v` put at `k` where the heap has room for it, growing the map as
/// `try_push` grows a vector.
fn try_insert(out: &mut HashMap<[i64; 3], u32>, k: [i64; 3], v: u32) -> Result<(), TryReserveError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))?;
    }
    out.insert(k, v);
    Ok(())
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
