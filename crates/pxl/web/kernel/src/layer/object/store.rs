// pxl-kernel/layer/object/store.rs

//! Object storage + spatial index. No "fine at current counts" here — this
//! is sized to work the same way at a handful of objects and at millions:
//! every mutation touches exactly the one L1 cell it affects (never a
//! whole-world walk), and every query is scoped to a cell or chunk (never a
//! scan over every placed object anywhere).
//!
//! The records are the second half: one per L2 chunk column holding user
//! objects — a sparse octree per 64³ band whose leaves are catalog values —
//! and, derived from them a
//! chunk at a time, the chunk's entries as three lanes and an index of the
//! cells they fall in. A coordinate is lowered here to a chunk key, a band
//! and a lattice position, and what comes back is lanes. What this module
//! adds is the eraser's two virtual edits, the change log's bumps, and the
//! persisted form in the host's storage.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use base64::Engine;
use pxl_render::mesh::paint::object::adapter::{Carve, Entries};
use pxl_spec::bake::constant::{band_cell_z, band_of, in_cell, pack_pos, unpack_pos, BandZ, CellZ, BAND_CELL_SHAMT, BAND_UNITS, CELL, OBJECT_FINEST, OBJECT_LAYOUT, POS_MASK, STEP_UNITS, U_PER_CELL};
use pxl_spec::bake::ground::shown_ground;
use pxl_spec::cell::object::{decor_id_from_num, decor_num_id, object_value_is_air, object_value_is_decor, placeable_cube_num_id, placeable_cube_of, terrain_style_cube_source, PlaceableCube};
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::hierarchy::{at_walk_order, child_of, first_descendant_of, parent_of, walk_order, Scale};
use pxl_spec::space::indexer::{FROM_L2_IDX, FROM_L3_IDX, L2, L2_IDX, L3, L3_IDX};
use pxl_spec::space::pvf3::fvec::FVec;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pvf3::Ivec;
use pxl_spec::space::pyramid::TOP_LEVEL;
use pxl_spec::space::sparse_tree::mem;
use pxl_spec::space::sparse_tree::tree::{self as sparse_tree, SparseTree};
use pxl_spec::space::xy::TO_XY;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::global::Global;
use crate::host::{now, storage_get, storage_set};
use crate::kernel::kernel;
use crate::layer::terrain::elevation::get_ground_type;
use crate::shell::setting::store::hidden_ground;
use crate::world::change::{bump_world, wake_at, world_changes, world_edge, OBJECT_COLUMN};
use crate::world::chunk::node::{chunk_sources_changed, column_written};
use crate::world::frame::coord::local_world_to_canonical;

/// What a leaf holds, past its corner and size. A solid is the catalog's own
/// cube; the id it is spelled out under belongs to the two sides' shared
/// dictionary (spec/cell), not here.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Solid { cube: PlaceableCube },
    Decor { decor_id: DecorId },
    Air,
}

/// Where an entry is, which is also what names it: the column it is filed
/// under, its band, and its packed position in that band — the three numbers
/// the kernel keys by. A holder of one reads the entry back, hides it or
/// erases it without a reverse lookup, which is what the formatted id this
/// replaced carried as text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryAt {
    pub col_idx: IVec,
    pub band_idx: BandZ,
    pub pos: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectLayerEntry {
    pub at: EntryAt,
    pub i: IVec,
    pub f: FVec,
    pub size: f64,
    pub kind: EntryKind,
}

pub type PlacedObject = ObjectLayerEntry;
pub type PlacedDecor = ObjectLayerEntry;
pub type ObjectAir = ObjectLayerEntry;
/// Anything a placement/erase/selection query should actually treat as "a
/// real thing occupying this slot" — a solid color cube or a decor object
/// alike, just never `EntryKind::Air` (that's a virtual erase-marker, not a
/// placed thing). collision.rs, geometry.rs and camera/ray.rs only ever touch the
/// shared fields (i/f/size/at).
pub type OccupiedEntry = ObjectLayerEntry;

// A page is five bare lanes — x, y, z, size, value — with no record header
// (the persistence section below). A blob of an older version is not read. Bump this again whenever a future change makes an older
// entry's encoding mean something different from what it meant when it was
// written.
pub const OBJECT_STORAGE_VERSION: u32 = 10;

fn storage_key() -> String {
    format!("pxl:object:v{OBJECT_STORAGE_VERSION}")
}

// ---- Chunk storage ----
//
// One record per L2 chunk column that holds user objects, and inside it one
// sparse octree per 64³ band — the unified cell format of
// issue/20260911-decor-identity-orientation-and-one-cell-format.md. What a
// coordinate is keyed by — the chunk (the L2 index of the XY column), the
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

/// The same key from the renderer's side, a page index and a page offset.
/// Both are normalised to XY, so a caller cannot reopen the gap `column_of`
/// closes by handing in an offset that still carries Z.
pub fn object_col_idx_of(page_idx: IVec, page_off: IVec) -> IVec {
    L2_IDX(TO_XY(ivec::add(FROM_L3_IDX(TO_XY(page_idx)), TO_XY(page_off))))
}

/// The state stored at `coord`, VOID where nothing is.
fn read_state(coord: &Pvf3) -> u32 {
    let at = At::of(coord);
    kernel().objects.read(at.col_idx, at.band, at.step)
}

// ---- Entries ----
//
// What a chunk's trees hold, read back as one entry per leaf — a leaf larger
// than a cell split into the cells it covers, since everything downstream
// (collision's 3×3×3 neighbourhood, a size never past one cell) assumes an
// entry is at most a cell. The kernel keeps them per chunk as three lanes —
// the band, a packed position, the state. A tree walked depth first visits a
// cell's leaves together, so a cell's entries are one contiguous run.
//
// `pos` packs the leaf's band-local position in finest-tier steps and its
// scale as the tiers above the finest (pxl-spec bake/constant.rs `pack_pos`).

/// One entry as a visit sees it. The chunk-local corner, size and state are
/// plain numbers; `corner()`, `cell()` and `at()` are worked out when asked
/// for, because few callers need them.
#[derive(Clone, Debug, Default)]
pub struct EntryCursor {
    /// Corner relative to the column's base (z = 0), in world units.
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
    /// The catalog value: a cube colour, AIR, or a decor kind.
    pub state: u32,

    at: At,
    pos: u32,
    /// False for the carve preview, which is not filed anywhere.
    stored: bool,
}

impl EntryCursor {
    /// Points the cursor at a column; `load` then reads its entries.
    pub fn at_chunk(&mut self, col_idx: IVec) {
        self.at.col_idx = col_idx;
    }

    pub fn load(&mut self, band_idx: BandZ, pos: u32, state: u32) {
        self.put(band_idx, pos, state);
        self.stored = true;
    }

    /// The carve preview, which is an entry the store would write but has not.
    fn load_carve(&mut self, carve: EntryAt) {
        self.at_chunk(carve.col_idx);
        self.put(carve.band_idx, carve.pos, AIR);
        self.stored = false;
    }

    fn put(&mut self, band_idx: BandZ, pos: u32, state: u32) {
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

    /// Where the entry is filed; None for the carve preview, which is not.
    pub fn at(&self) -> Option<EntryAt> {
        self.stored.then_some(EntryAt {
            col_idx: self.at.col_idx,
            band_idx: self.at.band,
            pos: self.pos,
        })
    }

    /// The entry as an object, for the few callers that keep one — every one
    /// of them reading entries out of the lanes, never the carve preview.
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
                cube: placeable_cube_of(self.state).expect("a solid leaf holds a cube the catalog gives out"),
            }
        };
        ObjectLayerEntry {
            at: self.at().unwrap_or_default(),
            i,
            f,
            size: self.size,
            kind,
        }
    }
}

// ---- Cells ----
//
// A cell's entries are one run of its chunk's lanes, found where the cell
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
/// entries again inside the call, and announces every column changed.
fn apply(writes: impl IntoIterator<Item = Write>) {
    for col_idx in kernel().objects.write(writes) {
        chunk_sources_changed();
        bump_world(OBJECT_COLUMN, col_idx);
        column_written(col_idx);
    }
}

/// A cursor loaded at an address, for the two readers that hold one: the
/// cell it falls in, and the corner and size a write back to it needs.
fn cursor_at(at: EntryAt) -> EntryCursor {
    let mut cursor = EntryCursor::default();
    cursor.at_chunk(at.col_idx);
    cursor.load(at.band_idx, at.pos, 0);
    cursor
}

/// Whether any entry — including the carve preview — is filed under a
/// column. One lookup; what a coarse build asks before skipping the scan.
pub fn col_has_entries(col_idx: IVec) -> bool {
    if kernel().objects.entries(col_idx).is_some() {
        return true;
    }
    store_state().virtual_carve_preview.as_ref().is_some_and(|c| c.at.col_idx == col_idx)
}

/// Every entry of a column. `render` applies the eraser preview's two edits,
/// the hidden entry left out and the carve preview put in, which only the
/// drawn build may see (see set_virtual_hide).
pub fn each_col_entry(col_idx: IVec, visit: &mut dyn FnMut(&EntryCursor), render: bool) {
    let mut cursor = EntryCursor::default();
    let hide = store_state().virtual_hide;
    if let Some(e) = kernel().objects.entries(col_idx) {
        cursor.at_chunk(col_idx);
        for k in 0..e.band.len() {
            cursor.load(e.band[k], e.pos[k], e.state[k]);
            if render && hide.is_some_and(|h| cursor.at() == Some(h)) {
                continue;
            }
            visit(&cursor);
        }
    }
    let carve = store_state().virtual_carve_preview;
    if let Some(carve) = carve.filter(|c| render && c.at.col_idx == col_idx) {
        cursor.load_carve(carve.at);
        visit(&cursor);
    }
}

fn each_cell_entry(ix: i64, iy: i64, iz: i64, visit: &mut dyn FnMut(&EntryCursor)) {
    let Some(run) = cell_run(ix, iy, iz) else { return };
    let col_idx = column_of([ix, iy, iz]);
    let Some(e) = kernel().objects.entries(col_idx) else { return };
    let mut cursor = EntryCursor::default();
    cursor.at_chunk(col_idx);
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

/// Whether anything really occupies the cell — what scattered decor asks
/// before standing in it, without making an object to find out.
pub fn cell_is_occupied(ix: i64, iy: i64, iz: i64) -> bool {
    cell_run(ix, iy, iz).is_some_and(|run| run.solid > 0)
}

/// Objects near a candidate corner+size — the cell it's in plus the 26
/// neighbors, since a candidate near a cell face can legitimately need to see
/// an object one cell over (collision.rs's find_overlapping_object, ray.rs's
/// resolve_hit_object). 27 O(1) lookups, still independent of total object
/// count.
pub fn get_objects_near(corner: &Pvf3, _size: f64) -> Vec<OccupiedEntry> {
    let [cx, cy, cz] = CELL.index_of(corner);
    // size can push the candidate's own far corner into the NEXT cell too —
    // covered by the same 3x3x3 neighborhood as long as size <= one cell
    // (object/size.rs's own largest tier), which every placeable size is.
    let mut out = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                occupants(cx + dx, cy + dy, cz + dz, &mut out);
            }
        }
    }
    out
}

/// One object by its address, or None — the address says which cell it is
/// in, so this reads one cell's entries, not every object's.
pub fn get_placed_object(at: EntryAt) -> Option<OccupiedEntry> {
    let [ix, iy, iz] = cursor_at(at).cell();
    let mut found = None;
    each_cell_entry(ix, iy, iz, &mut |entry| {
        if found.is_none() && !object_value_is_air(entry.state) && entry.at() == Some(at) {
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
}

static STORE: Global<StoreState> = Global::new();

fn store_state() -> &'static mut StoreState {
    STORE.get(|| StoreState {
        virtual_hide: None,
        hide_in: (false, false),
        virtual_carve_preview: None,
        save_due_ms: None,
    })
}

/// Makes exactly one placed object paint as if it didn't exist, for the real
/// GPU-mesh build ONLY — the eraser's hover preview, which needs the real
/// target to stop rendering (so what is behind it shows) with a translucent
/// ghost of it on top.
///
/// Deliberately does NOT affect the cell queries (collision, decor's
/// occupancy) or a raycast paint: the object has not been removed, and the
/// raycaster is what resolves the hover in the first place — if it stopped
/// seeing the hidden object, hover would clear, the hide would lift, the
/// raycaster would hit it again: an infinite loop.
///
/// Nor is it a change to what is stored: no revision moves, so nothing that
/// reads the store — a save, the boxes a view measures heights by — sees
/// the hover. A chunk sees it as one of its sources (`render_edits`), in
/// its tiers or its decor by the entry's kind, so hover churn never
/// rebakes the wrong half.
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
    let lowered = At::of(at);
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
/// beside the store (world/chunk/table.rs `Sources`): the hidden entry, with
/// whether it is drawn in tiers and in decor, and the carve preview.
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
    let hide = st.virtual_hide.map(|h| (h.col_idx, h.band_idx, h.pos));
    let carve = st.virtual_carve_preview.map(|c| (c.at.col_idx, Carve { band: c.at.band_idx, pos: c.at.pos }));
    kernel().sched.builder.set_edits(hide, carve);
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
// value the store writes (a block colour, AIR, a decor kind). Nothing else is
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
    each_col_entry(
        col_idx,
        &mut |entry| {
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
        },
        false,
    );
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
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(blob)
        .map_err(|_| MarshalError::new(Errno::EINVAL, Some("objects: the stored blob is not base64url"), None))?;
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

/// Reads the saved blob into the store. An unreadable blob starts the world
/// empty, as a missing one does.
pub fn load() {
    if let Some(raw) = storage_get(&storage_key()) {
        if decode_objects(&raw).is_err() {
            kernel().objects.clear();
        }
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
            storage_set(&storage_key(), &blob);
        }
    }
}

/// Saves what is pending now: the page hiding.
pub fn flush_save() {
    if store_state().save_due_ms.take().is_none() {
        return;
    }
    if let Ok(blob) = encode_objects() {
        storage_set(&storage_key(), &blob);
    }
}

// ---- Mutations ----

/// i/f is taken as-is — whatever camera/ray.rs's world_adjacent_corner
/// already computed (the same coordinate the hover preview and overlap check
/// used), never re-derived or re-snapped here. One source of truth for where
/// a placement goes, not a second one that quietly overrides it.
///
/// The kernel writes the leaf, compacts the band and drops it — and the chunk
/// — once every leaf in it reads back VOID: a redundant place() and a
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
    let at = At::of(coord);
    Write {
        col_idx: at.col_idx,
        band_idx: at.band,
        at: at.step,
        scale: Scale::of_side(size),
        state: val,
    }
}

/// Whether painting `color_id` at this exact whole cell would actually change
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
/// channel alone cancelled a bare-ground cube into a road.
///
/// And the cell must already hold an explicit AIR leaf, so the placement is
/// undoing a carve rather than adding a voxel terrain does not have. A ground
/// type is a per-column (2.5D) fact while a placement lands in the empty cell
/// above the surface, so without this every cube stacked on matching ground
/// read as redundant and was silently dropped.
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube, show_structures: bool) -> bool {
    if size < U_PER_CELL {
        return false;
    }
    let Some(source) = terrain_style_cube_source(cube) else { return false };
    let shown = shown_ground(|ch| get_ground_type(at, ch).unwrap_or(0), hidden_ground(show_structures));
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
pub fn place(at: &Pvf3, size: f64, cube: PlaceableCube, show_structures: bool) {
    let val = if place_value_is_redundant(at, size, cube, show_structures) {
        VOID
    } else {
        placeable_cube_num_id(cube)
    };
    write_object_value(at, size, val);
}

/// Same write path as place(), just tagged with a decor kind instead of a
/// color.
pub fn place_decor(at: &Pvf3, size: f64, decor_id: DecorId) {
    write_object_value(at, size, decor_num_id(decor_id));
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
// One record per L2 chunk column that holds user objects — a sparse
// octree per 64³ band whose leaves are catalog values — and, derived from
// it a chunk at a time, the chunk's entries as three
// lanes (band, packed position, state) and an index of the cells they
// fall in. Everything here is numbers: the client lowers a coordinate to
// a chunk key, a band and a lattice position before it crosses, and gets
// lanes back as views.
//
// A leaf is the catalog value itself, which the catalog keeps within a
// leaf's width (pxl-spec cell/object.rs); an entry is at most a cell because everything downstream — the collision neighbourhood,
// the raycast's per-cell runs — assumes one, and a leaf covering more is
// split into the cells it covers as the lanes are built; and the index is
// rebuilt a chunk at a time because a write touches one chunk and the cost
// is bounded by what that chunk holds.

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

pub(crate) struct Chunk {
    /// The band trees by Z, each leaf the catalog value it holds — VOID, a
    /// zeroed leaf, being the absence.
    bands: BTreeMap<BandZ, SparseTree<u16>>,
    /// The entries: the band trees walked in band order and each tree's walk
    /// order, a leaf coarser than a cell split into its cells in that order,
    /// so a cell's entries are one run and the lanes are sorted by
    /// (band, cell's walk order). Made again whenever a write changes the
    /// chunk, inside the write.
    pub(crate) e_band: Vec<BandZ>,
    pub(crate) e_pos: Vec<u32>,
    pub(crate) e_state: Vec<u32>,
    /// The lowest and highest any entry reaches, in units: the column's box
    /// on Z, made with the entries.
    z: (f64, f64),
}

impl Chunk {
    /// Re-reads the trees into the entry lanes: one entry per leaf, a leaf
    /// larger than a cell split into the cells it covers.
    fn derive_entries(&mut self) {
        self.e_band.clear();
        self.e_pos.clear();
        self.e_state.clear();
        let mut reach = (f64::INFINITY, f64::NEG_INFINITY);
        for (&bz, tree) in &self.bands {
            sparse_tree::walk(tree, &mut |x, y, z, scale, value| {
                let (x, y, z, val) = (x as u32, y as u32, z as u32, u32::from(value));
                if val == VOID || val == UNKNOWN {
                    return;
                }
                let lo = bz as f64 * BAND_UNITS + z as f64 * STEP_UNITS;
                reach = (reach.0.min(lo), reach.1.max(lo + scale.side()));
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
        }
        self.z = reach;
    }
}

/// The run [start, start + count) of a chunk's entries in one cell, and how
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
    /// A chunk for every column holding an object, and only for those: a
    /// band is dropped once every leaf of it reads VOID, and a chunk with its
    /// last band.
    chunks: BTreeMap<IVec, Chunk>,
    /// The lowest and highest the objects under a node of the hierarchy
    /// reach, in units, for every node over a column that holds any: keyed
    /// by the node's levels above a column and its coordinate at them. A
    /// column's is its own and a node's the union of its four children's,
    /// so a write makes again the one chain over the column it changed.
    reaches: HashMap<(u32, [i64; 2]), (f64, f64)>,
}

/// Levels of `Store::reaches` over a column: to the pyramid's top page, the
/// largest node a view measures.
const REACH_LEVELS: u32 = L3.above(L2) as u32 + TOP_LEVEL;

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Store {
        Store {
            chunks: BTreeMap::new(),
            reaches: HashMap::new(),
        }
    }

    /// The columns holding objects.
    pub fn cols(&self) -> impl Iterator<Item = IVec> + '_ {
        self.chunks.keys().copied()
    }

    /// The lowest and highest the objects under a node reach, in units: a
    /// node of the hierarchy by its side and its coordinate, a column's or
    /// coarser. None where nothing stands under it.
    pub fn reach_over(&self, (side, at): (Scale, [i64; 2])) -> Option<(f64, f64)> {
        self.reaches.get(&(u32::try_from(side.above(L2)).ok()?, at)).copied()
    }

    /// Makes again the reaches over column `col`, from it up.
    fn reach_up(&mut self, col: IVec) {
        let mut at = [ivec::X(col), ivec::Y(col)];
        match self.chunks.get(&col) {
            Some(chunk) => self.reaches.insert((0, at), chunk.z),
            None => self.reaches.remove(&(0, at)),
        };
        for level in 1..=REACH_LEVELS {
            at = parent_of(at);
            let under = (0..4).filter_map(|slot| self.reaches.get(&(level - 1, child_of(at, slot))));
            match under.copied().reduce(|a, b| (a.0.min(b.0), a.1.max(b.1))) {
                Some(z) => self.reaches.insert((level, at), z),
                None => self.reaches.remove(&(level, at)),
            };
        }
    }

    /// The state at a lattice position of band `band_idx` of column
    /// `col_idx`; VOID where nothing is.
    pub fn read(&self, col_idx: IVec, band_idx: BandZ, at: [u32; 3]) -> u32 {
        let Some(tree) = self.chunks.get(&col_idx).and_then(|c| c.bands.get(&band_idx)) else {
            return VOID;
        };
        u32::from(mem::get::<Ivec, u16>(&tree.mem, &tree.layout, band_key(at), 0))
    }

    /// Applies every write, then makes again the entries of each column one
    /// changed, once: what a reader sees is the trees, whatever it reads.
    /// The columns changed, for the caller to announce.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> BTreeSet<IVec> {
        let mut changed = BTreeSet::new();
        for w in writes {
            if self.put(w) {
                changed.insert(w.col_idx);
            }
        }
        for &col_idx in &changed {
            if let Some(chunk) = self.chunks.get_mut(&col_idx) {
                chunk.derive_entries();
            }
            self.reach_up(col_idx);
        }
        changed
    }

    /// Writes one leaf, compacts its band, and drops the band — and the
    /// chunk — once every leaf reads VOID. False where nothing was there to
    /// change: VOID written where no band is.
    fn put(&mut self, w: Write) -> bool {
        let Write { col_idx, band_idx, at, scale, state } = w;
        assert!((OBJECT_FINEST..=CELL).contains(&scale), "an object no finer than its finest tier and no coarser than a cell");
        let value = u16::try_from(state).expect("a catalog value fits a leaf (pxl-spec cell/object.rs)");
        let has_band = self.chunks.get(&col_idx).is_some_and(|c| c.bands.contains_key(&band_idx));
        if !has_band && state == VOID {
            return false;
        }
        let chunk = self.chunks.entry(col_idx).or_insert_with(|| Chunk {
            bands: BTreeMap::new(),
            e_band: Vec::new(),
            e_pos: Vec::new(),
            e_state: Vec::new(),
            z: (0.0, 0.0),
        });
        let tree = chunk.bands.entry(band_idx).or_insert_with(|| sparse_tree::alloc::<u16>(OBJECT_LAYOUT, 64));
        mem::set::<Ivec, u16>(&mut tree.mem, &tree.layout, band_key(at), value, scale, 0);
        sparse_tree::compact(tree);
        if band_is_empty(tree) {
            chunk.bands.remove(&band_idx);
            if chunk.bands.is_empty() {
                self.chunks.remove(&col_idx);
            }
        }
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
        let chunk = self.chunks.get(&col_idx)?;
        Some(Entries {
            band: &chunk.e_band,
            pos: &chunk.e_pos,
            state: &chunk.e_state,
        })
    }

    /// The run of a chunk's entries in the cell at band-local (cx, cy) and
    /// absolute cell Z `cz`: found where it lies in the lanes' order.
    pub fn cell(&self, col_idx: IVec, cx: u32, cy: u32, cz: CellZ) -> Option<CellRun> {
        let chunk = self.chunks.get(&col_idx)?;
        let (band_idx, z) = band_of(cz as i64);
        let want = Self::cell_order(band_idx, cx, cy, z);
        let order = |k: usize| {
            let (x, y, z, _) = unpack_pos(chunk.e_pos[k]);
            let ([x, y, z], _) = in_cell([x, y, z]);
            Self::cell_order(chunk.e_band[k], x, y, z)
        };
        let n = chunk.e_band.len();
        let start = partition_point(n, |k| order(k) < want);
        let end = start + partition_point(n - start, |k| order(start + k) == want);
        (end > start).then(|| CellRun {
            start: start as u32,
            count: (end - start) as u32,
            solid: (start..end).filter(|&k| chunk.e_state[k] != AIR).count() as u32,
        })
    }

    /// Every record gone, as a load that could not be read leaves the world.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.reaches.clear();
    }
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
