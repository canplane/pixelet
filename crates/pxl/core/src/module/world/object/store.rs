// pxl_core/module/world/object/store.rs

//! Object storage + spatial index. No "fine at current counts" here — this
//! is sized to work the same way at a handful of objects and at millions:
//! every mutation touches exactly the one L1 cell it affects (never a
//! whole-world walk), and every query is scoped to a cell or chunk (never a
//! scan over every placed object anywhere).
//!
//! The records are the kernel's (crates/pxl/core/src/world/object.rs): one per
//! L2 chunk column holding user objects — a palette, and one sparse octree
//! per 64³ band whose leaves are palette indices — and, derived from them a
//! chunk at a time, the chunk's entries as three lanes and an index of the
//! cells they fall in. A coordinate is lowered here to a chunk key, a band
//! and a lattice position, and what comes back is lanes. What this module
//! adds is the eraser's two virtual edits, the change log's bumps, and the
//! persisted form in the host's storage.

use pxl_shared::bake::constant::{OBJECT_LAYOUT, U_PER_CELL};
use pxl_shared::bake::ground::shown_ground;
use pxl_shared::record::placement::{encode_placements, Placement, PlacementTable};
use pxl_shared::record::table::RecordSource;
use pxl_shared::spec::cell::{
    cube_types, decor_id_from_num, decor_num_id, object_value_is_air, object_value_is_decor, placeable_cube_num_id, placeable_cube_of, terrain_style_cube_source,
    DecorId, PlaceableCube,
};
use pxl_shared::spec::space::indexer::{FROM_L2_IDX, FROM_L3_IDX, L2_IDX, L2_SHAMT, L3_IDX};
use pxl_shared::spec::space::pvf3::fvec::{self, FVec};
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::io::marshal::tree::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_core::kit::encoding::{b64url_to_bytes, bytes_to_b64url};
use xpute_core::kit::hash::fnv1a64_bytes;
use xpute_core::sys::error::MarshalError;

use crate::bake::terrain::js_round;
use crate::global::{now, Global};
use crate::host::{storage_get, storage_set};
use crate::kernel::{kernel, reindex_object};
use crate::module::world::change::{bump_world, request_world_tick, world_changes, DECOR_COLUMN, OBJECT_COLUMN};
use crate::module::world::frame::coord::local_world_to_canonical;
use crate::module::world::terrain::elevation::get_ground_type;
use crate::world::object::{unpack_pos, BandZ, Carve, CELL_STEPS, VOID};

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
/// placed thing). collision.rs/geometry.rs/camera/ray.rs only ever touch the
/// shared fields (i/f/size/at).
pub type OccupiedEntry = ObjectLayerEntry;

// v8 (2026-09-17): a terrain-styled cube's leaf value is its channel and its
// type (spec/cell's TERRAIN_STYLE_BASE), not a number counted out as the
// catalog was walked. A v7 blob's terrain-styled values are 1..32 in walk
// order and name different cubes under the new rule, which is exactly the
// case this version exists for.
//
// v7 (2026-09-13): one Placement record per page in a page-keyed container
// (the persistence section below) — the same form every placed thing has.
//
// Not migrated. Bump this again whenever a future change makes an older
// entry's encoding mean something different from what it meant when it was
// written.
pub const OBJECT_STORAGE_VERSION: u32 = 8;

fn storage_key() -> String {
    format!("pxl:object:v{OBJECT_STORAGE_VERSION}")
}

// ---- Chunk storage ----
//
// One record per L2 chunk column that holds user objects, and inside it one
// sparse octree per 64³ band — the unified cell format of
// issue/20260911-decor-identity-orientation-and-one-cell-format.md. What
// this section does is lower a coordinate to what the kernel keys by — the
// chunk (the L2 index of the XY column), the band's Z, and the position within
// the band in finest-tier steps, which is the low bits of each lane of the
// coordinate shifted to the finest level — and read its lanes back.

/// Cells along one side of a chunk, and of a band.
const CHUNK_CELLS: i64 = 1 << L2_SHAMT;
const BAND_STEP_MASK: i64 = CELL_STEPS as i64 * CHUNK_CELLS - 1;

fn cell_size_fp() -> i64 {
    pvf3::from_real(U_PER_CELL)
}

/// Floor division: the X axis is signed/cyclic (indexer.rs), and the divisor
/// is always positive, so a Euclidean quotient is the floor.
fn floor_div(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

/// The cell a coordinate is in — for callers that need to probe exact-cell
/// occupancy directly (get_objects_in_cell) instead of get_objects_near's
/// ±1-cell neighborhood, e.g. decor, which must tell "no object in this
/// exact ground cell" apart from "an object one cell over".
pub fn cell_index(coord: &PVF3) -> [i64; 3] {
    let fp = cell_size_fp();
    [floor_div(pvf3::X(coord), fp), floor_div(pvf3::Y(coord), fp), floor_div(pvf3::Z(coord), fp)]
}

/// Absolute PVF3 corner (min-x/y/z) of cell (ix,iy,iz).
fn cell_corner(ix: i64, iy: i64, iz: i64) -> PVF3 {
    let fp = cell_size_fp();
    let mut out = PVF3::default();
    pvf3::gather(&mut out, ix * fp, iy * fp, iz * fp);
    out
}

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
fn col_idx_of_cell(ix: i64, iy: i64, iz: i64) -> IVec {
    L2_IDX(TO_XY(cell_corner(ix, iy, iz).i))
}

/// The same key from the renderer's side, a page index and a page offset.
/// Both are normalised to XY, so a caller cannot reopen the gap above by
/// handing in an offset that still carries Z.
pub fn object_col_idx_of(page_idx: IVec, page_off: IVec) -> IVec {
    L2_IDX(TO_XY(ivec::add(FROM_L3_IDX(TO_XY(page_idx)), TO_XY(page_off))))
}

fn band_z_of(iz: i64) -> i32 {
    floor_div(iz, CHUNK_CELLS) as i32
}

/// A coordinate as the kernel keys it: its column, band, and position within
/// the band in finest-tier steps.
struct At {
    col_idx: IVec,
    band: i32,
    x: u32,
    y: u32,
    z: u32,
}

fn lower(coord: &PVF3) -> At {
    let mut lattice = PVF3::default();
    pvf3::sh(&mut lattice, coord, -OBJECT_LAYOUT.min);
    At {
        col_idx: L2_IDX(TO_XY(coord.i)),
        band: band_z_of(cell_index(coord)[2]),
        x: (ivec::X(lattice.i) & BAND_STEP_MASK) as u32,
        y: (ivec::Y(lattice.i) & BAND_STEP_MASK) as u32,
        z: (ivec::Z(lattice.i) & BAND_STEP_MASK) as u32,
    }
}

/// The state stored at `coord`, VOID where nothing is.
fn read_state(coord: &PVF3) -> u32 {
    let at = lower(coord);
    kernel().objects.read(at.col_idx, at.band, [at.x, at.y, at.z])
}

/// A chunk's entry lanes, valid until the next write or reindex; None where
/// the chunk holds nothing.
fn entry_lanes(col_idx: IVec) -> Option<(&'static [i32], &'static [u32], &'static [u32])> {
    let lanes = kernel().objects.chunk_lanes(col_idx)?;
    if lanes.count == 0 {
        return None;
    }
    let n = lanes.count as usize;
    // SAFETY: the store's own lanes, `count` long, untouched until the next
    // write or reindex, which no caller makes while it holds them.
    unsafe {
        Some((
            core::slice::from_raw_parts(lanes.band_at, n),
            core::slice::from_raw_parts(lanes.pos_at, n),
            core::slice::from_raw_parts(lanes.state_at, n),
        ))
    }
}

fn entry_count(col_idx: IVec) -> u32 {
    kernel().objects.chunk_lanes(col_idx).map_or(0, |l| l.count)
}

/// Real-unit local offset (x,y,z at min-level lattice units) as a PVF3,
/// computed via exact bit-shift (pvf3::sh), not a float multiply.
fn local_offset(x: u32, y: u32, z: u32) -> PVF3 {
    let mut scalar = PVF3::default();
    pvf3::gather(&mut scalar, pvf3::from_real(x as f64), pvf3::from_real(y as f64), pvf3::from_real(z as f64));
    let mut out = PVF3::default();
    pvf3::sh(&mut out, &scalar, OBJECT_LAYOUT.min);
    out
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
// level above the finest; the kernel's `unpack_pos` reads it back.

/// One finest-tier step, and one band's height, in world units.
fn step_units() -> f64 {
    2f64.powi(OBJECT_LAYOUT.min)
}

fn band_units() -> f64 {
    CHUNK_CELLS as f64 * 2f64.powi(OBJECT_LAYOUT.max)
}

/// The one entry that is not stored — the eraser's carve preview.
#[derive(Clone, Debug)]
struct Fixed {
    i: IVec,
    f: FVec,
}

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

    col_idx: IVec,
    cx0: i64,
    cy0: i64,
    band_idx: BandZ,
    pos: u32,
    x: u32,
    y: u32,
    z: u32,
    level: i32,
    fixed: Option<Fixed>,
}

impl EntryCursor {
    /// Points the cursor at a column; `load` then reads its entries.
    pub fn at_chunk(&mut self, col_idx: IVec) {
        let [cx0, cy0, _] = cell_index(&PVF3 { i: FROM_L2_IDX(col_idx), f: 0 });
        self.col_idx = col_idx;
        self.cx0 = cx0;
        self.cy0 = cy0;
    }

    pub fn load(&mut self, band_idx: BandZ, pos: u32, state: u32) {
        self.fixed = None;
        self.band_idx = band_idx;
        self.pos = pos;
        (self.x, self.y, self.z, self.level) = unpack_pos(pos);
        self.lx = self.x as f64 * step_units();
        self.ly = self.y as f64 * step_units();
        self.lz = band_idx as f64 * band_units() + self.z as f64 * step_units();
        self.size = 2f64.powi(self.level);
        self.state = state;
    }

    /// An entry given by its absolute corner rather than read from lanes.
    pub fn load_fixed(&mut self, col_idx: IVec, at: &PVF3, size: f64, state: u32) {
        self.fixed = Some(Fixed { i: at.i, f: at.f });
        self.col_idx = col_idx;
        let local = ivec::sub(at.i, FROM_L2_IDX(col_idx));
        self.lx = ivec::to_real(ivec::X(local)) + fvec::to_real(fvec::X(at.f));
        self.ly = ivec::to_real(ivec::Y(local)) + fvec::to_real(fvec::Y(at.f));
        self.lz = ivec::to_real(ivec::Z(local)) + fvec::to_real(fvec::Z(at.f));
        self.size = size;
        self.state = state;
    }

    /// The cell the entry is in.
    pub fn cell(&self) -> [i64; 3] {
        let steps = CELL_STEPS;
        [
            self.cx0 + (self.x / steps) as i64,
            self.cy0 + (self.y / steps) as i64,
            self.band_idx as i64 * CHUNK_CELLS + (self.z / steps) as i64,
        ]
    }

    /// The absolute corner.
    pub fn corner(&self) -> PVF3 {
        if let Some(fixed) = &self.fixed {
            return PVF3 { i: fixed.i, f: fixed.f };
        }
        let mut out = PVF3::default();
        pvf3::add(&mut out, &cell_corner(self.cx0, self.cy0, self.band_idx as i64 * CHUNK_CELLS), &local_offset(self.x, self.y, self.z));
        out
    }

    /// Where the entry is filed; None for the carve preview, which is not.
    pub fn at(&self) -> Option<EntryAt> {
        if self.fixed.is_some() {
            return None;
        }
        Some(EntryAt {
            col_idx: self.col_idx,
            band_idx: self.band_idx,
            pos: self.pos,
        })
    }

    /// The entry as an object, for the few callers that keep one — every one
    /// of them reading entries out of the lanes, never the carve preview.
    pub fn object(&self) -> ObjectLayerEntry {
        let PVF3 { i, f } = self.corner();
        let kind = if object_value_is_decor(self.state) {
            EntryKind::Decor { decor_id: decor_id_from_num(self.state) }
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

// ---- Index ----
//
// Cells by their corner, each naming the run of its chunk's entries it
// covers — the kernel's, rebuilt a chunk at a time by reindex_chunk, which
// then bumps the revisions of the column and of every cell that changed.

#[derive(Clone, Copy, Debug, Default)]
struct CellRun {
    start: u32,
    count: u32,
    solid: u32,
}

/// The run [start, start + count) of the column's entries in the cell, and
/// how many of them really occupy it — every kind but AIR. None where the
/// cell holds nothing.
fn cell_run(ix: i64, iy: i64, iz: i64) -> Option<CellRun> {
    let col_idx = col_idx_of_cell(ix, iy, iz);
    let [cx0, cy0, _] = cell_index(&PVF3 { i: FROM_L2_IDX(col_idx), f: 0 });
    let rec = kernel().objects.cell(col_idx, (ix - cx0) as u32, (iy - cy0) as u32, iz as i32)?;
    Some(CellRun {
        start: rec.start,
        count: rec.count,
        solid: rec.solid,
    })
}

/// Moves on every write to any column, so a reader that cached a derived
/// answer can tell whether the store has changed under it at all.
pub fn objects_revision() -> u32 {
    let log = world_changes();
    log.tick(OBJECT_COLUMN) + log.tick(DECOR_COLUMN)
}

fn bump_col(col_idx: IVec, objects: bool, decor: bool) {
    if objects {
        bump_world(OBJECT_COLUMN, col_idx);
    }
    if decor {
        bump_world(DECOR_COLUMN, col_idx);
    }
}

/// Re-derives one chunk's entries and cells in the kernel, which records the
/// change itself — the column in both of its topics and every cell whose
/// entries changed — and asks for the frame that delivers it.
fn reindex_chunk(col_idx: IVec) {
    reindex_object(kernel(), col_idx);
    request_world_tick();
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
    if entry_count(col_idx) > 0 {
        return true;
    }
    store_state().virtual_carve_preview.as_ref().is_some_and(|c| c.col_idx == col_idx)
}

/// Every entry of a column. `render` applies the eraser preview's two edits,
/// the hidden entry left out and the carve preview put in, which only the
/// drawn build may see (see set_virtual_hide).
pub fn each_col_entry(col_idx: IVec, visit: &mut dyn FnMut(&EntryCursor), render: bool) {
    let mut cursor = EntryCursor::default();
    let hide = store_state().virtual_hide;
    if let Some((bands, poss, states)) = entry_lanes(col_idx) {
        cursor.at_chunk(col_idx);
        for k in 0..bands.len() {
            cursor.load(bands[k], poss[k], states[k]);
            if render && hide.is_some_and(|h| cursor.at() == Some(h)) {
                continue;
            }
            visit(&cursor);
        }
    }
    let carve = store_state().virtual_carve_preview.clone();
    if let Some(carve) = carve.filter(|c| render && c.col_idx == col_idx) {
        cursor.load_fixed(col_idx, &PVF3 { i: carve.i, f: carve.f }, carve.size, cube_types::common::air as u32);
        visit(&cursor);
    }
}

fn each_cell_entry(ix: i64, iy: i64, iz: i64, visit: &mut dyn FnMut(&EntryCursor)) {
    let Some(run) = cell_run(ix, iy, iz) else { return };
    let col_idx = col_idx_of_cell(ix, iy, iz);
    let Some((bands, poss, states)) = entry_lanes(col_idx) else { return };
    let mut cursor = EntryCursor::default();
    cursor.at_chunk(col_idx);
    for k in run.start as usize..(run.start + run.count) as usize {
        cursor.load(bands[k], poss[k], states[k]);
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
pub fn get_objects_near(corner: &PVF3, _size: f64) -> Vec<OccupiedEntry> {
    let [cx, cy, cz] = cell_index(corner);
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
#[derive(Clone, Debug)]
struct CarvePreview {
    cell: [i64; 3],
    col_idx: IVec,
    i: IVec,
    f: FVec,
    size: f64,
}

struct StoreState {
    /// The one object painted as absent from the real render build — see
    /// set_virtual_hide.
    virtual_hide: Option<EntryAt>,
    virtual_carve_preview: Option<CarvePreview>,
    /// Set while a blob is being replayed into the store, so the writes it
    /// makes neither save (the blob is what is being loaded) nor reindex per
    /// row (the load reindexes every chunk once, after).
    restoring: bool,
    /// When the pending save is due, if one is.
    save_due_ms: Option<f64>,
}

static STORE: Global<StoreState> = Global::new();

fn store_state() -> &'static mut StoreState {
    STORE.get(|| StoreState {
        virtual_hide: None,
        virtual_carve_preview: None,
        restoring: false,
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
/// raycaster would hit it again: an infinite loop ("Maximum update depth
/// exceeded", 2026-09-03). Bumps the column's revision, the same path a real
/// place()/remove() takes.
pub fn set_virtual_hide(at: Option<EntryAt>) {
    let st = store_state();
    if st.virtual_hide == at {
        return;
    }
    let previous = st.virtual_hide.take();
    st.virtual_hide = at;
    send_render_edits();
    if let Some(previous) = previous {
        bump_for_object(previous);
    }
    if let Some(at) = at {
        bump_for_object(at);
    }
}

/// Bumps only the revision relevant to the entry's kind — decor or cube
/// content — so hover churn never rebakes the wrong half; both when the kind
/// is gone already (erased while hidden), which costs one extra bump on each
/// side.
fn bump_for_object(at: EntryAt) {
    let kind = get_placed_object(at).map(|o| o.kind);
    let is_decor = matches!(kind, Some(EntryKind::Decor { .. }));
    let is_solid = matches!(kind, Some(EntryKind::Solid { .. }));
    bump_col(at.col_idx, !is_decor, !is_solid);
}

/// The eraser's "dig into bare terrain" preview — the terrain-side twin of
/// virtual_hide_id. There is no leaf to hide when the hover target is real
/// terrain, so this injects one AIR entry into the render build that was
/// never written — same render-only scoping as virtual_hide_id, for the same
/// reason. `i`/`f`/`size` match what a real place_air() at this hover would
/// write, so the preview renders identically to what it stands in for.
pub fn set_virtual_carve_preview(at: &PVF3, size: f64) {
    let cell = cell_index(at);
    let [ix, iy, iz] = cell;
    let st = store_state();
    if st.virtual_carve_preview.as_ref().is_some_and(|c| c.cell == cell) {
        return;
    }
    let previous = st.virtual_carve_preview.take();
    let col_idx = col_idx_of_cell(ix, iy, iz);
    st.virtual_carve_preview = Some(CarvePreview {
        cell,
        col_idx,
        i: at.i,
        f: at.f,
        size,
    });
    send_render_edits();
    if let Some(previous) = previous {
        bump_col(previous.col_idx, true, false);
    }
    bump_col(col_idx, true, false);
}

pub fn clear_virtual_carve_preview() {
    let Some(carve) = store_state().virtual_carve_preview.take() else { return };
    send_render_edits();
    bump_col(carve.col_idx, true, false);
}

/// Tells the builds the two edits, before the bumps that rebuild the columns
/// they touch reach a chunk.
fn send_render_edits() {
    let st = store_state();
    let hide = st.virtual_hide.map(|h| (h.col_idx, h.band_idx, h.pos));
    let carve = st.virtual_carve_preview.as_ref().map(|c| {
        let mut cursor = EntryCursor::default();
        cursor.load_fixed(c.col_idx, &PVF3 { i: c.i, f: c.f }, c.size, cube_types::common::air as u32);
        (
            c.col_idx,
            Carve {
                lx: cursor.lx,
                ly: cursor.ly,
                lz: cursor.lz,
                size: c.size,
            },
        )
    });
    kernel().sched.builder.set_edits(hide, carve);
}

/// Cube-content revision of a column — a decor-only change never bumps it.
pub fn object_col_revision(col_idx: IVec) -> u32 {
    world_changes().revision_of(OBJECT_COLUMN, col_idx as i64)
}

/// Decor revision of a column — a solid-only change never bumps it.
pub fn decor_col_revision(col_idx: IVec) -> u32 {
    world_changes().revision_of(DECOR_COLUMN, col_idx as i64)
}

// ---- Persistence ----

// One Placement record per page, in a container that names the page:
//   [page_idx u64, placements pkt, page_idx u64, placements pkt, ...]
// The record is pxl_shared::record::placement's — the one form every placed
// thing has — with positions page-local to FROM_L3_IDX(page_idx). A row's
// value is the catalog value the store writes (a block colour, AIR, a decor
// kind), which is its palette id; its id is the hash of the entry's address.
// The store carries no heading: a placed decor's facing is derived from its
// position (decor_kinds.rs's user_decor_pick), so the row's heading is
// written as zero and not read back.
//
// Positions round-trip exactly: the store's steps are binary fractions of a
// cell, and a page-local f32 holds any such value under 1024
// (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).

/// The columns holding objects, in the kernel's order — for a walk over
/// everything placed, which is what a save is.
pub fn object_cols() -> Vec<IVec> {
    let objects = &kernel().objects;
    (0..objects.chunk_count()).map(|i| objects.chunk_key(i)).collect()
}

/// The id a stored row carries: the FNV-1a of the entry's address, which is
/// what issue/20260913-everything-is-a-file-the-placement-and-the-palette.md
/// has an atom's identity be. Written for the record's id lane and never read
/// back — a load rebuilds the address from the position the row also carries.
fn id_of(at: EntryAt) -> u64 {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&at.col_idx.to_le_bytes());
    bytes[8..12].copy_from_slice(&at.band_idx.to_le_bytes());
    bytes[12..].copy_from_slice(&at.pos.to_le_bytes());
    fnv1a64_bytes(&bytes)
}

pub fn encode_objects() -> Result<String, MarshalError> {
    let mut by_page: Vec<(IVec, Vec<Placement>)> = Vec::new();
    for col_idx in object_cols() {
        let page_idx = TO_XY(L3_IDX(FROM_L2_IDX(col_idx)));
        let page_base = FROM_L3_IDX(page_idx);
        let slot = match by_page.iter().position(|(p, _)| *p == page_idx) {
            Some(slot) => slot,
            None => {
                by_page.push((page_idx, Vec::new()));
                by_page.len() - 1
            }
        };
        let rows = &mut by_page[slot].1;
        each_col_entry(
            col_idx,
            &mut |entry| {
                let corner = entry.corner();
                let local = PVF3 {
                    i: ivec::sub(corner.i, page_base),
                    f: corner.f,
                };
                rows.push(Placement {
                    id: entry.at().map_or(0, id_of),
                    x: pvf3::to_real(pvf3::X(&local)),
                    y: pvf3::to_real(pvf3::Y(&local)),
                    z: pvf3::to_real(pvf3::Z(&local)),
                    size: entry.size,
                    heading: 0.0,
                    value: entry.state,
                    scale: None,
                    rotation: None,
                    tags: None,
                });
            },
            false,
        );
    }
    let mut packets = Vec::new();
    for (page_idx, rows) in &by_page {
        if !rows.is_empty() {
            packets.push((*page_idx, encode_placements(rows)?));
        }
    }
    let mut tree = TreeView::new();
    let mut failed = None;
    tree.branch(Some(&mut |b| {
        for (page_idx, pkt) in &packets {
            b.u64(Some(*page_idx));
            if let Err(e) = b.graft(pkt.clone()) {
                failed = Some(e);
            }
        }
    }));
    if let Some(e) = failed {
        return Err(e);
    }
    let pkt = encoder().encode(
        &tree,
        TreeEncoderOptions {
            init_cap: Some(1 << 14),
            max_cap: None,
        },
    )?;
    Ok(bytes_to_b64url(&pkt))
}

pub fn decode_objects(blob: &str) -> Result<(), MarshalError> {
    let bytes = b64url_to_bytes(blob);
    let root = TreeReader::new(bytes)?.read_branch()?;
    store_state().restoring = true;
    let result = (|| {
        let mut k = 0;
        while k + 1 < root.len {
            let page_idx = root.at(k)?.get_bigint()? as u64;
            let page_base = FROM_L3_IDX(page_idx);
            let table = PlacementTable::new(RecordSource::Branch(root.at_branch(k + 1)?))?;
            for r in 0..table.count as usize {
                let local = [table.x.get(r) as f64, table.y.get(r) as f64, table.z.get(r) as f64];
                let mut corner = PVF3::default();
                local_world_to_canonical(&mut corner, &local, page_base);
                write_object_value(&corner, table.size.get(r) as f64, table.value.get(r));
            }
            k += 2;
        }
        Ok(())
    })();
    store_state().restoring = false;
    for col_idx in object_cols() {
        reindex_chunk(col_idx);
    }
    result
}

/// Reads the saved blob into the store and builds the index once: what the
/// store's module load did. An unreadable blob starts the world empty, as a
/// missing one does.
pub fn load() {
    if let Some(raw) = storage_get(&storage_key()) {
        if decode_objects(&raw).is_err() {
            kernel().objects.clear();
        }
    }
    // Initial index build — O(total objects), paid once.
    for base in object_cols() {
        reindex_chunk(base);
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

/// Saves if the pending save is due: the turn's, on the shell's heartbeat when
/// nothing else asks for one.
pub fn flush_save_if_due() {
    if store_state().save_due_ms.is_some_and(|due| now() >= due) {
        flush_save();
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

/// Chunks, bands and cells currently holding objects.
pub fn object_store_stats() -> (u32, u32, u32) {
    let o = &kernel().objects;
    (o.chunk_count() as u32, o.band_count() as u32, o.cell_count() as u32)
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
fn write_object_value(coord: &PVF3, size: f64, val: u32) {
    // size is always an exact power of two by construction (object/size.rs's
    // OBJECT_SIZES); the round only guards against float noise.
    let level = js_round(size.log2()) as i32;
    let at = lower(coord);
    kernel().objects.write(at.col_idx, at.band, [at.x, at.y, at.z], level, val);
    if store_state().restoring {
        return;
    }
    save();
    reindex_chunk(at.col_idx);
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
fn place_value_is_redundant(at: &PVF3, size: f64, cube: PlaceableCube, show_structures: bool) -> bool {
    if size < U_PER_CELL {
        return false;
    }
    let Some(source) = terrain_style_cube_source(cube) else { return false };
    let shown = shown_ground(|ch| get_ground_type(at, ch).unwrap_or(0), show_structures);
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
pub fn place(at: &PVF3, size: f64, cube: PlaceableCube, show_structures: bool) {
    let val = if place_value_is_redundant(at, size, cube, show_structures) { VOID } else { placeable_cube_num_id(cube) };
    write_object_value(at, size, val);
}

/// Same write path as place(), just tagged with a decor kind instead of a
/// color.
pub fn place_decor(at: &PVF3, size: f64, decor_id: &str) {
    write_object_value(at, size, decor_num_id(decor_id));
}

/// Eraser's own write path for its "nothing placed here" case — a real,
/// present leaf (cube_types::common::air) that means "nothing here, on
/// purpose," unlike remove() below, which deletes a leaf outright and lets
/// whatever's underneath (almost always real baked terrain) show through
/// again; that's no help against terrain itself, which has no object-layer
/// leaf of its own to delete.
pub fn place_air(at: &PVF3, size: f64) {
    write_object_value(at, size, cube_types::common::air as u32);
}

pub fn remove(at: EntryAt) {
    // The address is all a write back to VOID needs: the cursor turns it into
    // the corner and the size, the same way a read of that entry does.
    let cursor = cursor_at(at);
    write_object_value(&cursor.corner(), cursor.size, VOID);
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
