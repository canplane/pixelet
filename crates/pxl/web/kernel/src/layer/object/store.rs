// pxl-kernel/layer/object/store.rs

//! Placed objects: one record per L2 column, a sparse octree per 64³ band
//! whose leaves are catalog values, with the column's entries derived as
//! lanes in which a cell's entries are one contiguous run. Every mutation
//! touches one column and every query is scoped to a cell, column or box.

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
use pxl_spec::space::hierarchy::{ancestor_of, at_walk_order, child_of, first_descendant_of, offset_in, parent_of, walk_order, Scale};
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

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Solid { cube: PlaceableCube },
    Decor { decor_id: DecorId },
    Air,
}

pub use pxl_spec::bake::constant::EntryAt;

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectLayerEntry {
    pub at: EntryAt,
    pub i: IVec,
    pub f: FVec,
    pub size: f64,
    pub kind: EntryKind,
}

/// Never `EntryKind::Air`, which is the eraser's marker, not a placed thing.
pub type OccupiedEntry = ObjectLayerEntry;

/// The L2 index of the cell's XY column, Z dropped: the renderer asks for a
/// column with z = 0, so a key that kept Z left every object above 64u
/// stored and hit-tested but never drawn.
fn column_of(cell: [i64; 3]) -> IVec {
    L2_IDX(TO_XY(CELL.base_of(cell).i))
}

/// The cell at a column's corner, on z = 0.
fn column_cell(col_idx: IVec) -> [i64; 3] {
    CELL.index_of(&Pvf3 { i: FROM_L2_IDX(col_idx), f: 0 })
}

/// Where a corner is filed: column, band, and position in finest-tier steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct At {
    pub col_idx: IVec,
    pub band: BandZ,
    pub step: [u32; 3],
}

impl At {
    /// The corner lowered to the cube's own lattice, the only place a cube
    /// of that side can be.
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

    pub fn cell(self) -> [i64; 3] {
        let [x0, y0, _] = column_cell(self.col_idx);
        let ([cx, cy, cz], _) = in_cell(self.step);
        [x0 + i64::from(cx), y0 + i64::from(cy), band_cell_z(self.band) + i64::from(cz)]
    }

    pub fn corner(self) -> Pvf3 {
        let [x0, y0, _] = column_cell(self.col_idx);
        let mut out = Pvf3::default();
        pvf3::add(&mut out, &CELL.base_of([x0, y0, band_cell_z(self.band)]), &OBJECT_FINEST.base_of(self.step.map(i64::from)));
        out
    }

    /// Exact, since a step is a binary fraction of a unit.
    pub fn local(self) -> [f64; 3] {
        let [x, y, z] = self.step.map(|v| v as f64 * STEP_UNITS);
        [x, y, self.band as f64 * BAND_UNITS + z]
    }
}

fn read_state(coord: &Pvf3) -> u32 {
    let at = At::of(coord);
    kernel().objects.read(at.col_idx, at.band, at.step)
}

// A leaf larger than a cell is split into the cells it covers, since
// collision and the pick assume an entry is at most a cell.
#[derive(Clone, Debug, Default)]
pub struct EntryCursor {
    /// Corner relative to the column's base (z = 0).
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
    pub state: u32,

    at: At,
    pos: u32,
}

impl EntryCursor {
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

    pub fn cell(&self) -> [i64; 3] {
        self.at.cell()
    }

    pub fn corner(&self) -> Pvf3 {
        self.at.corner()
    }

    pub fn at(&self) -> EntryAt {
        EntryAt {
            col_idx: self.at.col_idx,
            band_idx: self.at.band,
            pos: self.pos,
        }
    }

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

fn cell_run(ix: i64, iy: i64, iz: i64) -> Option<CellRun> {
    let col_idx = column_of([ix, iy, iz]);
    let [cx0, cy0, _] = column_cell(col_idx);
    kernel().objects.cell(col_idx, (ix - cx0) as u32, (iy - cy0) as u32, iz as i32)
}

/// Moves on every write to any column.
pub fn objects_revision() -> u32 {
    world_changes().tick(OBJECT_COLUMN)
}

fn apply(writes: impl IntoIterator<Item = Write>) {
    for (col_idx, band_idx) in kernel().objects.write(writes) {
        chunk_sources_changed();
        bump_world(OBJECT_COLUMN, col_idx);
        band_written(col_idx, band_idx);
    }
}

fn cursor_at(at: EntryAt) -> EntryCursor {
    let mut cursor = EntryCursor::default();
    cursor.at_column(at.col_idx);
    cursor.load(at.band_idx, at.pos, 0);
    cursor
}

/// Every entry a column stores, without the eraser's render-only edits.
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

pub fn get_objects_in_cell(ix: i64, iy: i64, iz: i64) -> Vec<OccupiedEntry> {
    let mut out = Vec::new();
    occupants(ix, iy, iz, &mut out);
    out
}

/// Whether any leaf, air included, is written in the cell `at` lies in.
pub fn holds_entry(at: &Pvf3) -> bool {
    let [ix, iy, iz] = CELL.index_of(at);
    cell_run(ix, iy, iz).is_some()
}

pub fn objects_at(at: &Pvf3) -> Vec<OccupiedEntry> {
    let [ix, iy, iz] = CELL.index_of(at);
    get_objects_in_cell(ix, iy, iz)
}

/// Two cubes share space exactly when one's node lies under the other's,
/// since every cube sits on its own size's lattice.
pub fn cube_node(corner: &Pvf3, size: f64) -> (Scale, [i64; 3]) {
    let side = Scale::of_side(size);
    (side, side.index_of(corner))
}

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

#[derive(Clone, Copy, Debug)]
struct CarvePreview {
    cell: [i64; 3],
    at: EntryAt,
}

struct StoreState {
    /// The one object hidden from the drawn build, and whether it is drawn
    /// in tiers, decor, or (already gone) either.
    virtual_hide: Option<EntryAt>,
    hide_in: (bool, bool),
    virtual_carve_preview: Option<CarvePreview>,
    save_due_ms: Option<f64>,
    /// The saved blob and the turn it was asked on: read on a later one.
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

/// Hides one placed object from the drawn build only, for the eraser's hover
/// preview. The pick and the cell queries still see it: if the pick stopped
/// seeing it, hover would clear, the hide would lift and the pick would hit
/// it again, forever. No revision moves, so saves do not see the hover.
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

/// Injects one never-written AIR entry into the drawn build, where the
/// eraser hovers bare terrain and there is no leaf to hide. Render-only, as
/// set_virtual_hide is.
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

/// The drawn build's two edits, compared as part of a chunk's and a decor
/// cell's sources.
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

fn send_render_edits() {
    let st = store_state();
    kernel().sched.builder.set_edits(st.virtual_hide, st.virtual_carve_preview.map(|c| c.at));
    chunk_sources_changed();
}

/// Moves on every write to what the column stores, and on nothing else.
pub fn object_col_revision(col_idx: IVec) -> u32 {
    world_changes().revision_of(OBJECT_COLUMN, col_idx as i64)
}

// ---- Persistence ----
//
// Per page: [page_idx u64, x f32[], y f32[], z f32[], size f32[], value u32[]],
// positions page-local. A placed decor's facing is derived from its position,
// so it is not stored. The layout's version is in the storage key; a blob of
// another layout is never read. Positions round-trip exactly in f32.
pub fn object_cols() -> Vec<IVec> {
    let objects = &kernel().objects;
    objects.cols().collect()
}

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
    // One batch, so each column's entries are rebuilt once.
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

pub fn load() {
    file::open(root::STORAGE, STORAGE_KEYS[storage::OBJECTS as usize], Owner::Objects);
}

/// A failed open means no blob was kept: the world starts empty.
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

/// An unreadable blob starts the world empty, as a missing one does.
fn loaded() {
    let st = store_state();
    if !st.loading.as_ref().is_some_and(|(_, asked)| turn() > *asked) {
        return;
    }
    let (blob, _) = st.loading.take().or_bug(Errno::ENOTRECOVERABLE);
    if core::str::from_utf8(&blob).map_or(true, |raw| decode_objects(raw).is_err()) {
        // what decoded before the failure was already read
        for (col_idx, band_idx) in kernel().objects.clear() {
            chunk_sources_changed();
            bump_world(OBJECT_COLUMN, col_idx);
            band_written(col_idx, band_idx);
        }
    }
}

/// A save encodes the whole store, so it is put off to merge a burst of
/// writes; what is pending is flushed when the page hides.
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

pub fn flush_save_if_due() {
    loaded();
    match store_state().save_due_ms {
        Some(due) if now() >= due => saves().notify(),
        Some(due) => wake_at(due),
        None => {}
    }
}

/// Walks the columns on the turn quota. A walk the store moved under is
/// dropped and redone; the save counts as done only once written, so a page
/// hiding mid-walk still flushes it.
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

pub fn flush_save() {
    if store_state().save_due_ms.take().is_none() {
        return;
    }
    if let Ok(blob) = encode_objects() {
        keep(blob);
    }
}

// ---- Mutations ----

/// A write of VOID drops the band, and the column, once every leaf reads
/// VOID, so a redundant record never sits in a save.
fn write_object_value(coord: &Pvf3, size: f64, val: u32) {
    apply([write_of(coord, size, val)]);
    save();
}

fn write_of(coord: &Pvf3, size: f64, val: u32) -> Write {
    // Size is a power of two by construction; the nearest scale only guards
    // against float noise.
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

/// Whether a whole-cell terrain-styled pick only undoes a carve: the terrain
/// must *show* that ground there (shown_ground, as the shader resolves it;
/// matching a channel alone turned a bare-ground cube into road), and the
/// cell must already hold an AIR leaf (ground type is per column, so without
/// this every cube stacked on matching ground was dropped).
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube, structures: bool) -> bool {
    if size < U_PER_CELL {
        return false;
    }
    let Some(source) = terrain_style_cube_source(cube) else { return false };
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let Some(ground) = Access::now().at(at, FINEST).exact() else { return false };
    let shown = shown_ground(|ch| channel_of(ground.cell, ch) as u32, hidden_ground(structures));
    if shown.channel != source.channel || shown.type_id != source.terrain_num_id {
        return false;
    }
    object_value_is_air(read_state(at))
}

/// A redundant re-paint of a carve writes VOID, as remove() does.
pub fn place(at: &Pvf3, size: f64, cube: PlaceableCube, structures: bool) {
    let val = if place_value_is_redundant(at, size, cube, structures) {
        VOID
    } else {
        placeable_cube_num_id(cube)
    };
    write_object_value(at, size, val);
}

/// Placing the decor a scatter stands in a cell an erase emptied writes
/// VOID, so the scattered instance returns as it was.
pub fn place_decor(at: &Pvf3, size: f64, decor_id: DecorId) {
    let value = if restores_scattered(at, size, decor_id).is_some() { VOID } else { decor_num_id(decor_id) };
    write_object_value(at, size, value);
}

/// The scattered instance `place_decor` would bring back, for its preview.
pub fn restores_scattered(at: &Pvf3, size: f64, decor_id: DecorId) -> Option<Draw> {
    (size >= U_PER_CELL && object_value_is_air(read_state(at))).then(|| scatters_at(decor_num_id(decor_id), at))?
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: &Pvf3, size: f64) {
    write_object_value(at, size, cube_types::common::air as u32);
}

pub fn remove(at: EntryAt) {
    let cursor = cursor_at(at);
    write_object_value(&cursor.corner(), cursor.size, VOID);
}

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

fn band_key(at: [u32; 3]) -> IVec {
    ivec::gather(at[0] as i64, at[1] as i64, at[2] as i64)
}

fn band_is_empty(tree: &SparseTree<u16>) -> bool {
    let mut empty = true;
    sparse_tree::walk(tree, &mut |_, _, _, _, idx| empty &= idx == 0);
    empty
}

pub const VOID: u32 = cube_types::common::void as u32;
pub const AIR: u32 = cube_types::common::air as u32;
pub const UNKNOWN: u32 = cube_types::common::unknown as u32;
const _: () = assert!(VOID == 0);

pub(crate) struct Column {
    /// VOID, a zeroed leaf, is the absence.
    bands: BTreeMap<BandZ, SparseTree<u16>>,
    /// Sorted by (band, cell's walk order), so a cell's entries are one run.
    pub(crate) e_band: Vec<BandZ>,
    pub(crate) e_pos: Vec<u32>,
    pub(crate) e_state: Vec<u32>,
    /// Each band's entries' box, from the column's corner across and zero up.
    bounds: BTreeMap<BandZ, Aabb>,
}

impl Column {
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
                // In walk order, so the lanes stay sorted.
                let levels = scale.above(CELL) as u32;
                for k in 0..u64::from(CELL.count_in(scale)).pow(3) {
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

/// A run of a column's entries in one cell; `solid` counts all but AIR.
#[derive(Clone, Copy, Debug, Default)]
pub struct CellRun {
    pub start: u32,
    pub count: u32,
    pub solid: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Write {
    pub col_idx: IVec,
    pub band_idx: BandZ,
    pub at: [u32; 3],
    pub scale: Scale,
    pub state: u32,
}

pub struct Store {
    /// Only columns with a non-VOID leaf; an emptied band and column are dropped.
    columns: BTreeMap<IVec, Column>,
    /// For every cube from a band's up to `INDEX_TOP` holding any entry, the
    /// box its entries fill; a cube's is the union of its children's.
    bounds: HashMap<(Scale, [i64; 3]), Aabb>,
    /// What the objects fold to at every cell from a band's up to the top
    /// level's; under a band, the band's tree keeps it (`set_folded`).
    folds: HashMap<(Scale, [i64; 3]), u16>,
}

/// Majority over the 2×2×2 children (pxl-spec bake/fold.rs), empty counted as a value
/// and a non-voxel value as empty, so a lone cube goes at the first fold and
/// a wall keeps whole cells.
fn fold_cell(children: &[u16], scale: Scale) -> u16 {
    let v: [u16; 8] = core::array::from_fn(|k| if object_value_is_voxel(u32::from(children[k])) { children[k] } else { VOID as u16 });
    majority(v, scale.above(CELL))
}

const TOP_CELL: Scale = GROUND.scale(TOP_LEVEL);

/// The coarsest cube the index keeps: as tall as `REACH` and as wide as the
/// coarsest page, so any query box meets at most eight of them.
const INDEX_TOP: Scale = {
    let tall = REACH.max[2] - REACH.min[2];
    let wide = L3.up(TOP_LEVEL as i32).side();
    let mut s = BAND;
    while s.side() < tall || s.side() < wide {
        s = s.up(1);
    }
    s
};

fn within(inner: &Aabb, outer: &Aabb) -> bool {
    (0..3).all(|k| inner.min[k] >= outer.min[k] && inner.max[k] <= outer.max[k])
}

/// Rebuilds the chain over one cube up to `top`; a cube reducing to nothing
/// is removed.
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

fn grown(b: &Aabb, side: f64) -> Aabb {
    Aabb {
        min: b.min.map(|v| (v / side).floor() * side),
        max: b.max.map(|v| (v / side).ceil() * side),
    }
}

fn slab((o, d): (V3, V3), b: &Aabb, (t0, t1): (f64, f64)) -> Option<(f64, f64)> {
    let (enter, leave, _) = b.span_along(o, d)?;
    let (enter, leave) = (enter.max(t0), leave.min(t1));
    (enter <= leave).then_some((enter, leave))
}

/// Amanatides–Woo traversal of `side` cells; returns whether `each` stopped it.
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

    pub fn cols(&self) -> impl Iterator<Item = IVec> + '_ {
        self.columns.keys().copied()
    }

    /// A box holding every entry inside `query`, never none where there is
    /// some; a band's bound is cut to `query` and may include air.
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

    pub fn visit_in(&self, query: &Aabb, scale: Scale, each: &mut dyn FnMut([i64; 3], &Aabb)) {
        self.descend(query, scale, &mut |s, at, bound| {
            if s == scale {
                each(at, bound);
            }
            s != scale
        });
    }

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

    /// The columns under a node, sorted and deduplicated.
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

    /// Every cell of `scale` that may hold objects along the ray, in ray
    /// order, until `each` stops it. A cube is entered only where the ray
    /// crosses its bound grown to whole cells of `scale`.
    pub fn along_ray(&self, origin: V3, direction: V3, (t0, t1): (f64, f64), scale: Scale, each: &mut dyn FnMut([i64; 3]) -> bool) {
        let ray = (origin, direction);
        cells_along(ray, (t0, t1), INDEX_TOP.side(), &mut |at, span| self.along_ray_from(ray, span, scale, INDEX_TOP, at, each));
    }

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

    pub fn read(&self, col_idx: IVec, band_idx: BandZ, at: [u32; 3]) -> u32 {
        let Some(tree) = self.columns.get(&col_idx).and_then(|c| c.bands.get(&band_idx)) else {
            return VOID;
        };
        u32::from(mem::get::<Ivec, u16>(&tree.mem, &tree.layout, band_key(at), 0))
    }

    /// Applies every write and rebuilds each changed column's entries once.
    /// Returns the bands changed, a band being a z16 chunk's height.
    pub fn write(&mut self, writes: impl IntoIterator<Item = Write>) -> BTreeSet<(IVec, BandZ)> {
        let mut changed = BTreeSet::new();
        for w in writes {
            if self.put(w) {
                changed.insert((w.col_idx, w.band_idx));
            }
        }
        // Compacted after all writes, so a batch walks each changed band once.
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

    /// What the objects fold to in the ground cells `from..to` of `target`,
    /// at every height, with `hide` left out.
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

    /// False says nothing is placed in that band cell; true only that something may be.
    pub fn may_hold(&self, at: [i64; 3]) -> bool {
        let [x, y, band] = ancestor_of(at, BAND.above(CELL) as u32);
        self.columns.get(&ivec::gather(x, y, 0)).is_some_and(|c| c.bands.contains_key(&(band as BandZ)))
    }

    /// The cell over the highest band in the column of cell `[x, y]`.
    pub fn ceiling(&self, [x, y]: [i64; 2]) -> Option<i64> {
        let levels = BAND.above(CELL) as u32;
        let [cx, cy] = ancestor_of([x, y], levels);
        let band = *self.columns.get(&ivec::gather(cx, cy, 0))?.bands.keys().next_back()?;
        Some(first_descendant_of([i64::from(band) + 1], levels)[0])
    }

    /// VOID where nothing; a non-voxel value is returned as is.
    pub fn folded_value(&self, scale: Scale, at: [i64; 3]) -> u16 {
        if scale >= BAND {
            return self.folds.get(&(scale, at)).copied().unwrap_or(VOID as u16);
        }
        let levels = BAND.above(scale) as u32;
        let [x, y, band] = ancestor_of(at, levels);
        let Some(tree) = self.columns.get(&ivec::gather(x, y, 0)).and_then(|c| c.bands.get(&(band as BandZ))) else {
            return VOID as u16;
        };
        let key = band_key(first_descendant_of(offset_in(at, levels), scale.above(OBJECT_FINEST) as u32).map(|c| c as u32));
        tree.mem.data[mem::resolve::<Ivec, u16>(&tree.mem, &tree.layout, key, scale, 0) as usize]
    }

    /// Column `col_idx`'s voxel entries at `target`: a leaf finer than
    /// `target` folded to it, a coarser one kept whole. Painted cell by cell, a
    /// 1u cube came out as sixty-four sub-cell boxes the mesher never merges.
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

    fn fold_up(&mut self, col_idx: IVec, band_idx: BandZ) {
        let at = [ivec::X(col_idx), ivec::Y(col_idx), i64::from(band_idx)];
        let band = self.columns.get(&col_idx).and_then(|c| c.bands.get(&band_idx));
        let v = band.map(|t| fold_cell(&t.mem.data[..8], BAND)).filter(|&v| v != VOID as u16);
        propagate_up(&mut self.folds, (BAND, at), TOP_CELL, v, |scale, children| {
            let v = fold_cell(&children.map(|c| c.unwrap_or(VOID as u16)), scale);
            (v != VOID as u16).then_some(v)
        });
    }

    /// False where VOID was written where no band is.
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

    fn cell_order(band_idx: BandZ, cx: u32, cy: u32, cz: u32) -> (BandZ, u64) {
        (band_idx, walk_order([cx, cy, cz], BAND_CELL_SHAMT))
    }

    pub fn entries(&self, col_idx: IVec) -> Option<Entries<'_>> {
        let column = self.columns.get(&col_idx)?;
        Some(Entries {
            col_idx,
            band: &column.e_band,
            pos: &column.e_pos,
            state: &column.e_state,
        })
    }

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

    /// Returns the bands held.
    pub fn clear(&mut self) -> Vec<(IVec, BandZ)> {
        let held = self.columns.iter().flat_map(|(&col, c)| c.bands.keys().map(move |&band| (col, band))).collect();
        self.columns.clear();
        self.bounds.clear();
        self.folds.clear();
        held
    }
}

/// Grows by doubling, asking the heap only when full.
fn try_push<T>(out: &mut Vec<T>, v: T) -> Result<(), TryReserveError> {
    if out.len() == out.capacity() {
        out.try_reserve(out.len().max(16))?;
    }
    out.push(v);
    Ok(())
}

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
