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
//! adds is what is string-shaped: the leaf ids, the eraser's two virtual
//! edits, the change log's bumps, and the persisted form in the host's
//! storage.

use pxl_shared::bake::constant::{OBJECT_LAYOUT, U_PER_CELL};
use pxl_shared::bake::ground::shown_ground;
use pxl_shared::record::placement::{encode_placements, Placement, PlacementTable};
use pxl_shared::record::table::RecordSource;
use pxl_shared::spec::cell::{
    cube_types, decor_id_from_num, decor_num_id, is_terrain_style_cube, object_value_is_air, object_value_is_decor, placeable_cube_id_from_num, placeable_cube_num_id,
    terrain_style_cube_source, DecorId, PlaceableCubeId,
};
use pxl_shared::spec::space::indexer::{FROM_L3_IDX, L2_BASE, L2_SHAMT, L3_IDX};
use pxl_shared::spec::space::pvf3::fvec::{self, FVec};
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::io::marshal::tree::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_core::kit::encoding::{b64url_to_bytes, bytes_to_b64url};
use xpute_core::kit::hash::fnv1a64_str;
use xpute_core::sys::error::MarshalError;

use crate::bake::terrain::js_round;
use crate::global::{now, Global};
use crate::host::{storage_get, storage_set};
use crate::kernel::{kernel, reindex_object};
use crate::module::world::change::{bump_world, request_world_tick, world_changes, DECOR_COLUMN, OBJECT_COLUMN};
use crate::module::world::frame::coord::local_world_to_canonical;
use crate::module::world::terrain::elevation::get_ground_type;
use crate::world::object::Carve;

/// What a leaf holds, past its corner and size.
#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    Solid { color_id: PlaceableCubeId },
    Decor { decor_id: DecorId },
    Air,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectLayerEntry {
    pub id: String,
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
/// shared fields (i/f/size/id).
pub type OccupiedEntry = ObjectLayerEntry;

// v7 (2026-09-13): one Placement record per page in a page-keyed container
// (the persistence section below) — the same form every placed thing has.
// Not migrated. Bump this again whenever a future change makes an older
// entry's encoding mean something different from what it meant when it was
// written.
pub const OBJECT_STORAGE_VERSION: u32 = 7;

fn storage_key() -> String {
    format!("pxl:object:v{OBJECT_STORAGE_VERSION}")
}

// ---- Chunk storage ----
//
// One record per L2 chunk column that holds user objects, and inside it one
// sparse octree per 64³ band — the unified cell format of
// issue/20260911-decor-identity-orientation-and-one-cell-format.md. What
// this section does is lower a coordinate to what the kernel keys by — the
// chunk (L2_BASE of the XY column), the band's Z, and the position within
// the band in finest-tier steps, which is the low bits of each lane of the
// coordinate shifted to the finest level — and read its lanes back.

/// Cells along one side of a chunk, and of a band.
const CHUNK_CELLS: i64 = 1 << L2_SHAMT;
/// Finest-tier steps along one side of a cell, and of a band.
const CELL_STEPS: i64 = 1 << -OBJECT_LAYOUT.min;
const BAND_STEP_MASK: i64 = CELL_STEPS * CHUNK_CELLS - 1;
const VOID: u32 = cube_types::common::VOID as u32;

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

pub fn cell_key(ix: i64, iy: i64, iz: i64) -> String {
    format!("{ix},{iy},{iz}")
}

fn parse_cell_key(key: &str) -> Option<[i64; 3]> {
    let mut it = key.split(',').map(|p| p.parse::<i64>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

/// Absolute PVF3 corner (min-x/y/z) of cell (ix,iy,iz).
fn cell_corner(ix: i64, iy: i64, iz: i64) -> PVF3 {
    let fp = cell_size_fp();
    let mut out = PVF3::default();
    pvf3::gather(&mut out, ix * fp, iy * fp, iz * fp);
    out
}

/// The column a cell belongs to, as the key everything about objects is filed
/// under: L2_BASE of the cell's XY column.
///
/// A column, so Z is dropped before the base is taken. The renderer asks for
/// a column by an XY page index and a page offset built with z = 0, so a key
/// that kept the cell's own Z put every object above absolute z = 64u (about
/// 120 m, one L2 span) under a key nothing ever asked for: stored, indexed,
/// hit-tested and never drawn.
fn column_of_cell(ix: i64, iy: i64, iz: i64) -> IVec {
    L2_BASE(TO_XY(cell_corner(ix, iy, iz).i))
}

/// The same key from the renderer's side, a page index and a page offset.
/// Both are normalised to XY, so a caller cannot reopen the gap above by
/// handing in an offset that still carries Z.
pub fn object_column_of(page_idx: IVec, page_off: IVec) -> IVec {
    L2_BASE(TO_XY(ivec::add(FROM_L3_IDX(TO_XY(page_idx)), TO_XY(page_off))))
}

fn band_z_of(iz: i64) -> i32 {
    floor_div(iz, CHUNK_CELLS) as i32
}

/// A coordinate as the kernel keys it: its column, band, and position within
/// the band in finest-tier steps.
struct At {
    key: IVec,
    band: i32,
    x: u32,
    y: u32,
    z: u32,
}

fn lower(coord: &PVF3) -> At {
    let mut lattice = PVF3::default();
    pvf3::sh(&mut lattice, coord, -OBJECT_LAYOUT.min);
    At {
        key: L2_BASE(TO_XY(coord.i)),
        band: band_z_of(cell_index(coord)[2]),
        x: (ivec::X(lattice.i) & BAND_STEP_MASK) as u32,
        y: (ivec::Y(lattice.i) & BAND_STEP_MASK) as u32,
        z: (ivec::Z(lattice.i) & BAND_STEP_MASK) as u32,
    }
}

/// The state stored at `coord`, VOID where nothing is.
fn read_state(coord: &PVF3) -> u32 {
    let at = lower(coord);
    kernel().objects.read(at.key, at.band, [at.x, at.y, at.z])
}

/// A chunk's entry lanes, valid until the next write or reindex; None where
/// the chunk holds nothing.
fn entry_lanes(base: IVec) -> Option<(&'static [i32], &'static [u32], &'static [u32])> {
    let lanes = kernel().objects.chunk_lanes(base)?;
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

fn entry_count(base: IVec) -> u32 {
    kernel().objects.chunk_lanes(base).map_or(0, |l| l.count)
}

/// Deterministic id for a walked leaf block — encodes exactly what remove()
/// needs to locate it again (including its own cell key, so remove() never
/// needs a reverse lookup), no random/stored identity required.
fn leaf_id(key: &str, x: u32, y: u32, z: u32, level: i32) -> String {
    format!("{key}:{x},{y},{z}:{level}")
}

struct LeafId {
    key: String,
    x: u32,
    y: u32,
    z: u32,
    level: i32,
}

fn parse_leaf_id(id: &str) -> Option<LeafId> {
    let mut parts = id.split(':');
    let (key, pos, level) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    parse_cell_key(key)?;
    let mut p = pos.split(',').map(|v| v.parse::<u32>().ok());
    let (x, y, z) = (p.next()??, p.next()??, p.next()??);
    if p.next().is_some() {
        return None;
    }
    Some(LeafId {
        key: key.to_string(),
        x,
        y,
        z,
        level: level.parse().ok()?,
    })
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
// `pos` packs the leaf's band-local position in finest-tier steps — 0..511 on
// each axis, nine bits each — and its level above the finest in four, as
// object.rs's pack_pos does.

const POS_BITS: u32 = 9;
const POS_MASK: u32 = (1 << POS_BITS) - 1;

/// One finest-tier step, and one band's height, in world units.
fn step_units() -> f64 {
    2f64.powi(OBJECT_LAYOUT.min)
}

fn band_units() -> f64 {
    CHUNK_CELLS as f64 * 2f64.powi(OBJECT_LAYOUT.max)
}

fn pack_pos(x: u32, y: u32, z: u32, level: i32) -> u32 {
    x | (y << POS_BITS) | (z << (2 * POS_BITS)) | (((level - OBJECT_LAYOUT.min) as u32) << (3 * POS_BITS))
}

/// The one entry that is not stored — the eraser's carve preview.
#[derive(Clone, Debug)]
struct Fixed {
    i: IVec,
    f: FVec,
    id: String,
}

/// One entry as a visit sees it. The chunk-local corner, size and state are
/// plain numbers; `corner()`, `cell()` and `id()` are worked out when asked
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

    cx0: i64,
    cy0: i64,
    band: i32,
    pos: u32,
    x: u32,
    y: u32,
    z: u32,
    level: i32,
    fixed: Option<Fixed>,
}

impl EntryCursor {
    /// Points the cursor at a chunk's first cell; `load` then reads entries.
    pub fn at_chunk(&mut self, cx0: i64, cy0: i64) {
        self.cx0 = cx0;
        self.cy0 = cy0;
    }

    pub fn load(&mut self, band: i32, pos: u32, state: u32) {
        self.fixed = None;
        self.band = band;
        self.pos = pos;
        self.x = pos & POS_MASK;
        self.y = (pos >> POS_BITS) & POS_MASK;
        self.z = (pos >> (2 * POS_BITS)) & POS_MASK;
        self.level = (pos >> (3 * POS_BITS)) as i32 + OBJECT_LAYOUT.min;
        self.lx = self.x as f64 * step_units();
        self.ly = self.y as f64 * step_units();
        self.lz = band as f64 * band_units() + self.z as f64 * step_units();
        self.size = 2f64.powi(self.level);
        self.state = state;
    }

    /// An entry given by its absolute corner rather than read from lanes.
    pub fn load_fixed(&mut self, base: IVec, i: IVec, f: FVec, size: f64, state: u32, id: &str) {
        self.fixed = Some(Fixed { i, f, id: id.to_string() });
        let local = ivec::sub(i, base);
        self.lx = ivec::to_real(ivec::X(local)) + fvec::to_real(fvec::X(f));
        self.ly = ivec::to_real(ivec::Y(local)) + fvec::to_real(fvec::Y(f));
        self.lz = ivec::to_real(ivec::Z(local)) + fvec::to_real(fvec::Z(f));
        self.size = size;
        self.state = state;
    }

    /// The cell the entry is in.
    pub fn cell(&self) -> [i64; 3] {
        let steps = CELL_STEPS as u32;
        [
            self.cx0 + (self.x / steps) as i64,
            self.cy0 + (self.y / steps) as i64,
            self.band as i64 * CHUNK_CELLS + (self.z / steps) as i64,
        ]
    }

    /// The absolute corner.
    pub fn corner(&self) -> PVF3 {
        if let Some(fixed) = &self.fixed {
            return PVF3 { i: fixed.i, f: fixed.f };
        }
        let mut out = PVF3::default();
        pvf3::add(&mut out, &cell_corner(self.cx0, self.cy0, self.band as i64 * CHUNK_CELLS), &local_offset(self.x, self.y, self.z));
        out
    }

    pub fn id(&self) -> String {
        if let Some(fixed) = &self.fixed {
            return fixed.id.clone();
        }
        let [ix, iy, iz] = self.cell();
        leaf_id(&cell_key(ix, iy, iz), self.x, self.y, self.z, self.level)
    }

    /// The entry as the store's own two numbers, for a consumer that keeps
    /// instances as lanes and rebuilds the id on demand with entry_id_of. A
    /// fixed entry has no lane form and reports NO_ENTRY.
    pub fn band_of(&self) -> u32 {
        if self.fixed.is_some() {
            0xffff_ffff
        } else {
            self.band as u32
        }
    }

    pub fn pos_of(&self) -> u32 {
        if self.fixed.is_some() {
            0
        } else {
            self.pos
        }
    }

    /// The entry as an object, for the few callers that keep one.
    pub fn object(&self) -> ObjectLayerEntry {
        let PVF3 { i, f } = self.corner();
        let kind = if object_value_is_decor(self.state) {
            EntryKind::Decor { decor_id: decor_id_from_num(self.state) }
        } else if object_value_is_air(self.state) {
            EntryKind::Air
        } else {
            EntryKind::Solid {
                color_id: placeable_cube_id_from_num(self.state),
            }
        };
        ObjectLayerEntry {
            id: self.id(),
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
    let base = column_of_cell(ix, iy, iz);
    let [cx0, cy0, _] = cell_index(&PVF3 { i: base, f: 0 });
    let rec = kernel().objects.cell(base, (ix - cx0) as u32, (iy - cy0) as u32, iz as i32)?;
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

fn bump_column(base: IVec, objects: bool, decor: bool) {
    if objects {
        bump_world(OBJECT_COLUMN, base);
    }
    if decor {
        bump_world(DECOR_COLUMN, base);
    }
}

/// Re-derives one chunk's entries and cells in the kernel, which records the
/// change itself — the column in both of its topics and every cell whose
/// entries changed — and asks for the frame that delivers it.
fn reindex_chunk(base: IVec) {
    reindex_object(kernel(), base);
    request_world_tick();
}

/// The string id of the entry `(band, pos)` under column `base` — what a
/// consumer that kept an instance as lanes asks for when a hit on that
/// instance has to resolve to a selectable entry.
pub fn entry_id_of(base: IVec, band: u32, pos: u32) -> String {
    let [cx0, cy0, _] = cell_index(&PVF3 { i: base, f: 0 });
    let mut cursor = EntryCursor::default();
    cursor.at_chunk(cx0, cy0);
    cursor.load(band as i32, pos, 0);
    cursor.id()
}

/// Whether any entry — including the carve preview — is filed under a
/// column. One lookup; what a coarse build asks before skipping the scan.
pub fn column_has_entries(base: IVec) -> bool {
    if entry_count(base) > 0 {
        return true;
    }
    store_state().virtual_carve_preview.as_ref().is_some_and(|c| c.column == base)
}

/// Every entry of a column. `render` applies the eraser preview's two edits,
/// the hidden entry left out and the carve preview put in, which only the
/// drawn build may see (see set_virtual_hide).
pub fn each_column_entry(base: IVec, visit: &mut dyn FnMut(&EntryCursor), render: bool) {
    let mut cursor = EntryCursor::default();
    let hide = store_state().virtual_hide_id.clone();
    if let Some((bands, poss, states)) = entry_lanes(base) {
        let [cx0, cy0, _] = cell_index(&PVF3 { i: base, f: 0 });
        cursor.at_chunk(cx0, cy0);
        for k in 0..bands.len() {
            cursor.load(bands[k], poss[k], states[k]);
            if render && hide.as_deref().is_some_and(|h| cursor.id() == h) {
                continue;
            }
            visit(&cursor);
        }
    }
    let carve = store_state().virtual_carve_preview.clone();
    if let Some(carve) = carve.filter(|c| render && c.column == base) {
        cursor.load_fixed(base, carve.i, carve.f, carve.size, cube_types::common::AIR as u32, &carve.id);
        visit(&cursor);
    }
}

/// The drawn build's two edits to a column, as the kernel's paint takes
/// them: the hidden entry by its band and packed position — only when it is
/// this column's, since the same numbers name a different entry in every
/// other column — and the carve preview by its corner relative to the
/// column's base.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColumnEdits {
    pub hide: bool,
    pub hide_band: i32,
    pub hide_pos: u32,
    pub carve: bool,
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
}

pub fn column_render_edits(base: IVec) -> ColumnEdits {
    let mut edits = ColumnEdits::default();
    let st = store_state();
    if let Some(hide) = st.virtual_hide_id.as_deref().and_then(parse_leaf_id) {
        if let Some([ix, iy, iz]) = parse_cell_key(&hide.key) {
            if column_of_cell(ix, iy, iz) == base {
                edits.hide = true;
                edits.hide_band = band_z_of(iz);
                edits.hide_pos = pack_pos(hide.x, hide.y, hide.z, hide.level);
            }
        }
    }
    if let Some(carve) = st.virtual_carve_preview.as_ref().filter(|c| c.column == base) {
        let mut cursor = EntryCursor::default();
        cursor.load_fixed(base, carve.i, carve.f, carve.size, cube_types::common::AIR as u32, &carve.id);
        edits.carve = true;
        edits.lx = cursor.lx;
        edits.ly = cursor.ly;
        edits.lz = cursor.lz;
        edits.size = carve.size;
    }
    edits
}

fn each_cell_entry(ix: i64, iy: i64, iz: i64, visit: &mut dyn FnMut(&EntryCursor)) {
    let Some(run) = cell_run(ix, iy, iz) else { return };
    let base = column_of_cell(ix, iy, iz);
    let Some((bands, poss, states)) = entry_lanes(base) else { return };
    let [cx0, cy0, _] = cell_index(&PVF3 { i: base, f: 0 });
    let mut cursor = EntryCursor::default();
    cursor.at_chunk(cx0, cy0);
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

/// Every object claiming cell `key` — read live.
pub fn get_objects_in_cell(key: &str) -> Vec<OccupiedEntry> {
    let mut out = Vec::new();
    if let Some([ix, iy, iz]) = parse_cell_key(key) {
        occupants(ix, iy, iz, &mut out);
    }
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

/// One object by id, or None — the id carries its cell (leaf_id), so this is
/// one cell's entries, not every object's.
pub fn get_placed_object(id: &str) -> Option<OccupiedEntry> {
    let parsed = parse_leaf_id(id)?;
    let [ix, iy, iz] = parse_cell_key(&parsed.key)?;
    let mut found = None;
    each_cell_entry(ix, iy, iz, &mut |entry| {
        if found.is_none() && !object_value_is_air(entry.state) && entry.id() == id {
            found = Some(entry.object());
        }
    });
    found
}

/// The eraser's "dig into bare terrain" preview — see set_virtual_carve_preview.
#[derive(Clone, Debug)]
struct CarvePreview {
    key: String,
    column: IVec,
    i: IVec,
    f: FVec,
    size: f64,
    id: String,
}

struct StoreState {
    /// The one object painted as absent from the real render build — see
    /// set_virtual_hide.
    virtual_hide_id: Option<String>,
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
        virtual_hide_id: None,
        virtual_carve_preview: None,
        restoring: false,
        save_due_ms: None,
    })
}

/// Which entry set_virtual_hide is currently suppressing, for the one
/// consumer that has to keep drawing it in some form — decor, whose hit
/// testing runs against the very mesh the hide would remove.
pub fn virtual_hidden_id() -> Option<String> {
    store_state().virtual_hide_id.clone()
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
pub fn set_virtual_hide(id: Option<&str>) {
    let st = store_state();
    if st.virtual_hide_id.as_deref() == id {
        return;
    }
    let previous = st.virtual_hide_id.take();
    st.virtual_hide_id = id.map(str::to_string);
    send_render_edits();
    if let Some(previous) = previous {
        bump_for_object(&previous);
    }
    if let Some(id) = id {
        bump_for_object(id);
    }
}

/// Bumps only the revision relevant to `id`'s kind — decor or cube content —
/// so hover churn never rebakes the wrong half; both when the kind is gone
/// already (erased while hidden), which costs one extra bump on each side.
fn bump_for_object(id: &str) {
    let Some(parsed) = parse_leaf_id(id) else { return };
    let Some([ix, iy, iz]) = parse_cell_key(&parsed.key) else { return };
    let kind = get_placed_object(id).map(|o| o.kind);
    let is_decor = matches!(kind, Some(EntryKind::Decor { .. }));
    let is_solid = matches!(kind, Some(EntryKind::Solid { .. }));
    bump_column(column_of_cell(ix, iy, iz), !is_decor, !is_solid);
}

/// The eraser's "dig into bare terrain" preview — the terrain-side twin of
/// virtual_hide_id. There is no leaf to hide when the hover target is real
/// terrain, so this injects one AIR entry into the render build that was
/// never written — same render-only scoping as virtual_hide_id, for the same
/// reason. `i`/`f`/`size` match what a real place_air() at this hover would
/// write, so the preview renders identically to what it stands in for.
pub fn set_virtual_carve_preview(i: IVec, f: FVec, size: f64) {
    let [ix, iy, iz] = cell_index(&PVF3 { i, f });
    let key = cell_key(ix, iy, iz);
    let st = store_state();
    if st.virtual_carve_preview.as_ref().is_some_and(|c| c.key == key) {
        return;
    }
    let previous = st.virtual_carve_preview.take();
    let column = column_of_cell(ix, iy, iz);
    st.virtual_carve_preview = Some(CarvePreview {
        id: format!("virtual-carve:{key}"),
        key,
        column,
        i,
        f,
        size,
    });
    send_render_edits();
    if let Some(previous) = previous {
        bump_column(previous.column, true, false);
    }
    bump_column(column, true, false);
}

pub fn clear_virtual_carve_preview() {
    let Some(carve) = store_state().virtual_carve_preview.take() else { return };
    send_render_edits();
    bump_column(carve.column, true, false);
}

/// Tells the builds the two edits, before the bumps that rebuild the columns
/// they touch reach a chunk.
fn send_render_edits() {
    let st = store_state();
    let hide = st.virtual_hide_id.as_deref().and_then(parse_leaf_id).and_then(|h| {
        let [ix, iy, iz] = parse_cell_key(&h.key)?;
        Some((column_of_cell(ix, iy, iz), band_z_of(iz), pack_pos(h.x, h.y, h.z, h.level)))
    });
    let carve = st.virtual_carve_preview.as_ref().map(|c| {
        let mut cursor = EntryCursor::default();
        cursor.load_fixed(c.column, c.i, c.f, c.size, cube_types::common::AIR as u32, &c.id);
        (
            c.column,
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
pub fn object_column_revision(base: IVec) -> u32 {
    world_changes().revision_of(OBJECT_COLUMN, base as i64)
}

/// Decor revision of a column — a solid-only change never bumps it.
pub fn decor_column_revision(base: IVec) -> u32 {
    world_changes().revision_of(DECOR_COLUMN, base as i64)
}

// ---- Persistence ----

// One Placement record per page, in a container that names the page:
//   [page_idx u64, placements pkt, page_idx u64, placements pkt, ...]
// The record is pxl_shared::record::placement's — the one form every placed
// thing has — with positions page-local to FROM_L3_IDX(page_idx). A row's
// value is the catalog value the store writes (a block colour, AIR, a decor
// kind), which is its palette id; its id is the hash of the entry's own id.
// The store carries no heading: a placed decor's facing is derived from its
// position (decor_kinds.rs's user_decor_pick), so the row's heading is
// written as zero and not read back.
//
// Positions round-trip exactly: the store's steps are binary fractions of a
// cell, and a page-local f32 holds any such value under 1024
// (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).

/// The columns holding objects, in the kernel's order — for a walk over
/// everything placed, which is what a save is.
pub fn object_columns() -> Vec<IVec> {
    let objects = &kernel().objects;
    (0..objects.chunk_count()).map(|i| objects.chunk_key(i)).collect()
}

pub fn encode_objects() -> Result<String, MarshalError> {
    let mut by_page: Vec<(IVec, Vec<Placement>)> = Vec::new();
    for base in object_columns() {
        let page_idx = TO_XY(L3_IDX(base));
        let page_base = FROM_L3_IDX(page_idx);
        let slot = match by_page.iter().position(|(p, _)| *p == page_idx) {
            Some(slot) => slot,
            None => {
                by_page.push((page_idx, Vec::new()));
                by_page.len() - 1
            }
        };
        let rows = &mut by_page[slot].1;
        each_column_entry(
            base,
            &mut |entry| {
                let corner = entry.corner();
                let local = PVF3 {
                    i: ivec::sub(corner.i, page_base),
                    f: corner.f,
                };
                rows.push(Placement {
                    id: fnv1a64_str(&entry.id()),
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
                write_object_value(corner.i, corner.f, table.size.get(r) as f64, table.value.get(r));
            }
            k += 2;
        }
        Ok(())
    })();
    store_state().restoring = false;
    for base in object_columns() {
        reindex_chunk(base);
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
    for base in object_columns() {
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

/// Saves if the pending save is due: the tick's.
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
fn write_object_value(i: IVec, f: FVec, size: f64, val: u32) {
    // size is always an exact power of two by construction (object/size.rs's
    // OBJECT_SIZES); the round only guards against float noise.
    let level = js_round(size.log2()) as i32;
    let at = lower(&PVF3 { i, f });
    kernel().objects.write(at.key, at.band, [at.x, at.y, at.z], level, val);
    if store_state().restoring {
        return;
    }
    save();
    reindex_chunk(at.key);
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
fn place_value_is_redundant(i: IVec, f: FVec, size: f64, color_id: &str, show_structures: bool) -> bool {
    if size < U_PER_CELL || !is_terrain_style_cube(color_id) {
        return false;
    }
    let source = terrain_style_cube_source(color_id);
    let shown = shown_ground(|ch| get_ground_type(i, f, ch).unwrap_or(0), show_structures);
    if shown.channel != source.channel || shown.type_id != source.terrain_num_id {
        return false;
    }
    object_value_is_air(read_state(&PVF3 { i, f }))
}

/// Carving a cell (place_air) and then re-painting it with the exact
/// terrain-styled cube the real ground already is there cancels back out to
/// nothing: rather than storing a "solid" leaf that only ever repaints what
/// terrain would already show, this writes VOID — the same absence remove()
/// itself produces.
pub fn place(i: IVec, f: FVec, size: f64, color_id: &str, show_structures: bool) {
    let val = if place_value_is_redundant(i, f, size, color_id, show_structures) {
        VOID
    } else {
        placeable_cube_num_id(color_id)
    };
    write_object_value(i, f, size, val);
}

/// Same write path as place(), just tagged with a decor kind instead of a
/// color.
pub fn place_decor(i: IVec, f: FVec, size: f64, decor_id: &str) {
    write_object_value(i, f, size, decor_num_id(decor_id));
}

/// Eraser's own write path for its "nothing placed here" case — a real,
/// present leaf (cube_types::common::AIR) that means "nothing here, on
/// purpose," unlike remove() below, which deletes a leaf outright and lets
/// whatever's underneath (almost always real baked terrain) show through
/// again; that's no help against terrain itself, which has no object-layer
/// leaf of its own to delete.
pub fn place_air(i: IVec, f: FVec, size: f64) {
    write_object_value(i, f, size, cube_types::common::AIR as u32);
}

pub fn remove(id: &str) {
    let Some(parsed) = parse_leaf_id(id) else { return };
    let Some([ix, iy, iz]) = parse_cell_key(&parsed.key) else { return };
    // The id carries its cell and its band-local position (leaf_id), which is
    // all a write back to VOID needs: the band's corner, then the offset.
    let corner = cell_corner(floor_div(ix, CHUNK_CELLS) * CHUNK_CELLS, floor_div(iy, CHUNK_CELLS) * CHUNK_CELLS, floor_div(iz, CHUNK_CELLS) * CHUNK_CELLS);
    let mut abs = PVF3::default();
    pvf3::add(&mut abs, &corner, &local_offset(parsed.x, parsed.y, parsed.z));
    write_object_value(abs.i, abs.f, 2f64.powi(parsed.level), VOID);
}
