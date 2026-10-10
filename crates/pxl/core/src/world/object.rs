//! The object store's records and the object layer's paint:
//! module/world/object/store.ts's chunk storage, entries and cell index,
//! and reference/object_layer.ts's paint, in Rust.
//!
//! One record per L2 chunk column that holds user objects — a palette,
//! and one sparse octree per 64³ band whose leaves are palette indices —
//! and, derived from it a chunk at a time, the chunk's entries as three
//! lanes (band, packed position, state) and an index of the cells they
//! fall in. Everything here is numbers: the client lowers a coordinate to
//! a chunk key, a band and a lattice position before it crosses, and gets
//! lanes back as views. Why the palette is per chunk, why an entry is at
//! most a cell, and why the index is rebuilt a chunk at a time are the
//! TypeScript's, mirrored so a chunk's entries are the same lane for lane
//! (object_store.test.ts).

use std::collections::{BTreeMap, BTreeSet};

use pxl_shared::spec::space::pvf3::{ivec, Ivec};
use pxl_shared::spec::space::sparse_tree::mem;
use pxl_shared::spec::space::sparse_tree::tree::{self as sparse_tree, SparseTree, SparseTreeLayout};

use crate::bake::terrain::js_round;
use crate::bake::volume::{Cell, Volumes, Voxel};
use crate::bake::voxel::{pack_sub_cell_fraction, NO_LAYER, SUB_CELL_NONE};
use crate::space::constant::U_PER_CELL;
use crate::world::pool::Pool;

/// OBJECT_LAYOUT's finest and coarsest levels: an eighth of a cell, and a
/// cell. A band tree reaches six levels above that, to the band's 64 cells.
pub const MIN_LEVEL: i32 = -3;
pub const CELL_LEVEL: i32 = 0;
pub const BAND_SHAMT: u32 = 6;
pub const BAND_CELLS: u32 = 1 << BAND_SHAMT;
/// A band's tree: OBJECT_LAYOUT's levels, reaching six above the cell to
/// the band's 64 cells.
const BAND_LAYOUT: SparseTreeLayout = SparseTreeLayout {
    dim: 3,
    min: MIN_LEVEL,
    max: CELL_LEVEL + BAND_SHAMT as i32,
};
/// Finest-tier steps along one side of a cell, and one such step in
/// world units.
pub const CELL_STEPS: u32 = 1 << (-MIN_LEVEL);
pub const STEP_UNITS: f64 = 1.0 / CELL_STEPS as f64;
/// One band's height, in world units.
const BAND_UNITS: f64 = BAND_CELLS as f64;
/// An entry's packed position: nine bits an axis in finest-tier steps,
/// then its level above the finest in four.
pub const POS_BITS: u32 = 9;
const POS_MASK: u32 = (1 << POS_BITS) - 1;

pub fn pack_pos(x: u32, y: u32, z: u32, level: i32) -> u32 {
    x | (y << POS_BITS) | (z << (2 * POS_BITS)) | (((level - MIN_LEVEL) as u32) << (3 * POS_BITS))
}

/// A lattice position in a band, as its tree's key.
fn band_key(at: [u32; 3]) -> u64 {
    ivec::gather(at[0] as i64, at[1] as i64, at[2] as i64)
}

/// Whether every leaf of a band's tree reads VOID.
fn band_is_empty(tree: &SparseTree<u16>) -> bool {
    let mut empty = true;
    sparse_tree::walk(tree, &mut |_, _, _, _, idx| empty &= idx == 0);
    empty
}

/// The packed position's parts: x, y, z in finest-tier steps, and the level.
pub fn unpack_pos(pos: u32) -> (u32, u32, u32, i32) {
    (
        pos & POS_MASK,
        (pos >> POS_BITS) & POS_MASK,
        (pos >> (2 * POS_BITS)) & POS_MASK,
        (pos >> (3 * POS_BITS)) as i32 + MIN_LEVEL,
    )
}

/// cell/mod.ts's sentinels: the absence a zeroed leaf reads as, the
/// eraser's "nothing here, on purpose", and the value an entry never is.
pub const VOID: u32 = 0;
pub const AIR: u32 = 0x80;
pub const UNKNOWN: u32 = 0xff;

struct Chunk {
    key: u64,
    /// Index → state; index 0 is VOID in every chunk.
    palette: Vec<u32>,
    /// The band Z range the chunk's bands lie in; a band inside it may be gone.
    band_min: i32,
    band_max: i32,
    /// The entries, derived by `reindex`.
    e_band: Vec<i32>,
    e_pos: Vec<u32>,
    e_state: Vec<u32>,
}

/// A cell by its column and its absolute cell Z, and its band-local column.
type CellKey = (u64, i32, u32);

#[derive(Clone, Copy, Default)]
pub struct CellRec {
    pub start: u32,
    pub count: u32,
    /// Entries that really occupy the cell — every kind but AIR.
    pub solid: u32,
}

/// What a chunk's lanes are, for a reader that takes views.
pub struct ChunkLanes {
    pub count: u32,
    pub band_at: *const i32,
    pub pos_at: *const u32,
    pub state_at: *const u32,
}

pub struct Store {
    /// Chunks in the order they were opened, the last moved into a closed
    /// one's place — which is the order the persisted form lists them in.
    chunks: Vec<Chunk>,
    chunk_at: BTreeMap<u64, u32>,
    bands: BTreeMap<(u64, i32), SparseTree<u16>>,
    cells: BTreeMap<CellKey, CellRec>,
    /// Cells whose entries changed since the client last took them, as
    /// band-local (cx, cy) and absolute cell Z rows.
    touched: Vec<[i32; 3]>,
    touched_set: BTreeSet<CellKey>,
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Store {
        Store {
            chunks: Vec::new(),
            chunk_at: BTreeMap::new(),
            bands: BTreeMap::new(),
            cells: BTreeMap::new(),
            touched: Vec::new(),
            touched_set: BTreeSet::new(),
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunk_key(&self, i: usize) -> u64 {
        self.chunks[i].key
    }

    pub fn band_count(&self) -> usize {
        self.bands.len()
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    fn open_chunk(&mut self, key: u64) -> usize {
        if let Some(&pos) = self.chunk_at.get(&key) {
            return pos as usize;
        }
        let pos = self.chunks.len();
        self.chunks.push(Chunk {
            key,
            palette: vec![VOID],
            band_min: i32::MAX,
            band_max: i32::MIN,
            e_band: Vec::new(),
            e_pos: Vec::new(),
            e_state: Vec::new(),
        });
        self.chunk_at.insert(key, pos as u32);
        pos
    }

    /// Removes a chunk that has no bands left, moving the last one into its
    /// position.
    fn close_chunk(&mut self, pos: usize) {
        self.unindex_cells(pos);
        let gone = self.chunks.swap_remove(pos);
        self.chunk_at.remove(&gone.key);
        if pos < self.chunks.len() {
            self.chunk_at.insert(self.chunks[pos].key, pos as u32);
        }
    }

    fn open_band(&mut self, cpos: usize, z: i32) {
        let key = self.chunks[cpos].key;
        if self.bands.contains_key(&(key, z)) {
            return;
        }
        self.bands.insert((key, z), sparse_tree::alloc::<u16>(BAND_LAYOUT, 64, None));
        let chunk = &mut self.chunks[cpos];
        chunk.band_min = chunk.band_min.min(z);
        chunk.band_max = chunk.band_max.max(z);
    }

    /// Removes a band, then tightens its chunk's Z range — and removes the
    /// chunk when nothing is left in it.
    fn close_band(&mut self, cpos: usize, z: i32) {
        let key = self.chunks[cpos].key;
        if let Some(mut tree) = self.bands.remove(&(key, z)) {
            sparse_tree::free(&mut tree);
        }
        let chunk = &self.chunks[cpos];
        let mut lo = i32::MAX;
        let mut hi = i32::MIN;
        for bz in chunk.band_min..=chunk.band_max {
            if self.bands.contains_key(&(key, bz)) {
                lo = lo.min(bz);
                hi = hi.max(bz);
            }
        }
        if lo > hi {
            self.close_chunk(cpos);
            return;
        }
        let chunk = &mut self.chunks[cpos];
        chunk.band_min = lo;
        chunk.band_max = hi;
    }

    /// The state's index in the chunk's palette, appended if new.
    fn palette_index(&mut self, cpos: usize, state: u32) -> u16 {
        let palette = &mut self.chunks[cpos].palette;
        if let Some(k) = palette.iter().position(|&s| s == state) {
            return k as u16;
        }
        assert!(palette.len() < u16::MAX as usize, "a chunk's palette past a leaf's width");
        palette.push(state);
        (palette.len() - 1) as u16
    }

    /// The state at a lattice position of band `z` of chunk `key`; VOID
    /// where nothing is.
    pub fn read(&self, key: u64, z: i32, at: [u32; 3]) -> u32 {
        let Some(&cpos) = self.chunk_at.get(&key) else { return VOID };
        let Some(tree) = self.bands.get(&(key, z)) else { return VOID };
        self.chunks[cpos as usize].palette[mem::get::<Ivec, u16>(&tree.mem, &tree.layout, band_key(at), 0) as usize]
    }

    /// Writes `state` as one leaf of `level` at a lattice position of band
    /// `z` of chunk `key`, compacts the band, and drops the band — and the
    /// chunk — once every leaf reads VOID. The entries are not re-derived:
    /// that is `reindex`, once, after however many writes.
    pub fn write(&mut self, key: u64, z: i32, at: [u32; 3], level: i32, state: u32) {
        assert!((MIN_LEVEL..=CELL_LEVEL).contains(&level), "an object level outside its layout");
        let cpos = match self.chunk_at.get(&key) {
            Some(&pos) => pos as usize,
            None if state == VOID => return,
            None => self.open_chunk(key),
        };
        if !self.bands.contains_key(&(key, z)) {
            if state == VOID {
                return;
            }
            self.open_band(cpos, z);
        }
        let idx = self.palette_index(cpos, state);
        let tree = self.bands.get_mut(&(key, z)).expect("the band just opened");
        mem::set::<Ivec, u16>(&mut tree.mem, &tree.layout, band_key(at), idx, level, 0);
        sparse_tree::compact(tree);
        if band_is_empty(tree) {
            self.close_band(cpos, z);
        }
    }

    /// Re-derives one chunk's entries and cells; a chunk that is gone has
    /// nothing to derive, and its cells were unindexed as it closed.
    pub fn reindex(&mut self, key: u64) {
        let Some(&cpos) = self.chunk_at.get(&key) else { return };
        let cpos = cpos as usize;
        self.unindex_cells(cpos);
        self.derive_entries(cpos);
        self.index_cells(cpos);
    }

    /// Re-reads a chunk's trees into its entry lanes: one entry per leaf,
    /// a leaf larger than a cell split into the cells it covers.
    fn derive_entries(&mut self, cpos: usize) {
        let chunk = &mut self.chunks[cpos];
        chunk.e_band.clear();
        chunk.e_pos.clear();
        chunk.e_state.clear();
        for bz in chunk.band_min..=chunk.band_max {
            let Some(tree) = self.bands.get(&(chunk.key, bz)) else { continue };
            sparse_tree::walk(tree, &mut |x, y, z, level, idx| {
                let (x, y, z) = (x as u32, y as u32, z as u32);
                let val = chunk.palette[idx as usize];
                if val == VOID || val == UNKNOWN {
                    return;
                }
                if level <= CELL_LEVEL {
                    chunk.e_band.push(bz);
                    chunk.e_pos.push(pack_pos(x, y, z, level));
                    chunk.e_state.push(val);
                    return;
                }
                let span = 1u32 << (level - MIN_LEVEL);
                for dz in (0..span).step_by(CELL_STEPS as usize) {
                    for dy in (0..span).step_by(CELL_STEPS as usize) {
                        for dx in (0..span).step_by(CELL_STEPS as usize) {
                            chunk.e_band.push(bz);
                            chunk.e_pos.push(pack_pos(x + dx, y + dy, z + dz, CELL_LEVEL));
                            chunk.e_state.push(val);
                        }
                    }
                }
            });
        }
    }

    fn cell_of(key: u64, band: i32, pos: u32) -> CellKey {
        let (x, y, z, _) = unpack_pos(pos);
        (key, band * BAND_CELLS as i32 + (z / CELL_STEPS) as i32, ((y / CELL_STEPS) << BAND_SHAMT) | (x / CELL_STEPS))
    }

    fn touch(&mut self, cell: CellKey) {
        if self.touched_set.insert(cell) {
            self.touched.push([(cell.2 & (BAND_CELLS - 1)) as i32, (cell.2 >> BAND_SHAMT) as i32, cell.1]);
        }
    }

    /// Removes a chunk's cells from the index, reading its entry lanes as
    /// they were before the change that is being reindexed.
    fn unindex_cells(&mut self, cpos: usize) {
        let mut last: Option<CellKey> = None;
        for k in 0..self.chunks[cpos].e_band.len() {
            let chunk = &self.chunks[cpos];
            let cell = Self::cell_of(chunk.key, chunk.e_band[k], chunk.e_pos[k]);
            if last == Some(cell) {
                continue;
            }
            last = Some(cell);
            self.touch(cell);
            self.cells.remove(&cell);
        }
    }

    /// The entries are sorted by cell, so each cell's record is summed
    /// whole and written once.
    fn index_cells(&mut self, cpos: usize) {
        let mut open: Option<(CellKey, CellRec)> = None;
        for k in 0..self.chunks[cpos].e_band.len() {
            let chunk = &self.chunks[cpos];
            let cell = Self::cell_of(chunk.key, chunk.e_band[k], chunk.e_pos[k]);
            let solid = (chunk.e_state[k] != AIR) as u32;
            if open.is_none_or(|(c, _)| c != cell) {
                if let Some((c, rec)) = open.take() {
                    self.cells.insert(c, rec);
                }
                self.touch(cell);
                open = Some((cell, CellRec { start: k as u32, count: 0, solid: 0 }));
            }
            let rec = &mut open.as_mut().expect("the cell just opened").1;
            rec.count += 1;
            rec.solid += solid;
        }
        if let Some((c, rec)) = open {
            self.cells.insert(c, rec);
        }
    }

    /// The cells touched since the last take, as rows; the rows stay where
    /// they are until the next change.
    pub fn take_touched(&mut self) -> (*const i32, usize) {
        self.touched_set.clear();
        let n = self.touched.len();
        self.touched.clear();
        (self.touched.as_ptr() as *const i32, n)
    }

    /// A chunk's entry lanes, or none where the chunk is gone.
    pub fn chunk_lanes(&self, key: u64) -> Option<ChunkLanes> {
        let &cpos = self.chunk_at.get(&key)?;
        let chunk = &self.chunks[cpos as usize];
        Some(ChunkLanes {
            count: chunk.e_band.len() as u32,
            band_at: chunk.e_band.as_ptr(),
            pos_at: chunk.e_pos.as_ptr(),
            state_at: chunk.e_state.as_ptr(),
        })
    }

    /// The run of a chunk's entries in the cell at band-local (cx, cy) and
    /// absolute cell Z `cz`.
    pub fn cell(&self, key: u64, cx: u32, cy: u32, cz: i32) -> Option<CellRec> {
        self.cells.get(&(key, cz, (cy << BAND_SHAMT) | cx)).copied()
    }

    /// Every record gone, as a load that could not be read leaves the world.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.chunk_at.clear();
        self.bands.clear();
        self.cells.clear();
        self.touched.clear();
        self.touched_set.clear();
    }
}

// ---- Paint ----

/// What a state paints as: nothing (decor renders as instanced geometry,
/// not a voxel), air, or a solid with a material.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Untaught = 0,
    Solid = 1,
    Air = 2,
    Decor = 3,
}

/// The one entry that is not stored — the eraser's carve preview, at a
/// corner relative to the column's base, in world units.
#[derive(Clone, Copy)]
pub struct Carve {
    pub lx: f64,
    pub ly: f64,
    pub lz: f64,
    pub size: f64,
}

/// What a paint made: whether any voxel was set, and whether one was refused.
#[derive(Clone, Copy)]
pub struct Paint {
    pub any: bool,
    pub refused: bool,
}

/// A column's entries as voxels of a chunk volume — adapter.ts's paint.
pub struct Painter {
    kinds: Vec<Kind>,
    materials: Vec<u32>,
    layers: Vec<u8>,
    /// The voxels this paint has claimed, so a second sub-cell object
    /// landing on a claimed one is filed as overflow rather than
    /// overwriting it; by packed address, cleared per paint.
    claimed: BTreeSet<u32>,
}

const Z_BIAS: i32 = 1 << 19;

fn voxel_address(cx: u32, cy: u32, z: i32) -> u32 {
    (((z + Z_BIAS) as u32) << 12) | (cy << 6) | cx
}

impl Default for Painter {
    fn default() -> Self {
        Painter::new()
    }
}

impl Painter {
    pub fn new() -> Painter {
        Painter {
            kinds: Vec::new(),
            materials: Vec::new(),
            layers: Vec::new(),
            claimed: BTreeSet::new(),
        }
    }

    /// Teaches what `state` paints as. A state is what a leaf's palette
    /// index resolves to, so a state past a leaf's width is a bug in the
    /// catalogue and not taught, rather than sizing the tables to it.
    pub fn teach(&mut self, state: u32, kind: Kind, material: u32, layer: u8) {
        if state >= u16::MAX as u32 {
            return;
        }
        let i = state as usize;
        if i >= self.kinds.len() {
            self.kinds.resize(i + 1, Kind::Untaught);
            self.materials.resize(i + 1, 0);
            self.layers.resize(i + 1, NO_LAYER as u8);
        }
        self.kinds[i] = kind;
        self.materials[i] = material;
        self.layers[i] = layer;
    }

    /// What `state` paints as: its packed material and layer, or none when
    /// it paints nothing (untaught, air, decor).
    pub fn material_of(&self, state: u32) -> Option<(u32, u8)> {
        match self.kind(state) {
            Kind::Solid => Some((self.materials[state as usize], self.layers[state as usize])),
            _ => None,
        }
    }

    fn kind(&self, state: u32) -> Kind {
        self.kinds.get(state as usize).copied().unwrap_or(Kind::Untaught)
    }

    /// Paints the column `key` into `volume`: every stored entry but the
    /// one hidden (by its band and packed position), then the carve
    /// preview. Whether any voxel was painted, and whether a paint was
    /// refused — after which the rest is not painted, as the volume is
    /// not the composite that was asked for anyway.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(&mut self, store: &Store, volumes: &mut Volumes, pools: &mut [Pool], volume: usize, key: u64, hide: Option<(i32, u32)>, carve: Option<Carve>) -> Paint {
        self.claimed.clear();
        let mut any = false;
        if let Some(&cpos) = store.chunk_at.get(&key) {
            let chunk = &store.chunks[cpos as usize];
            for k in 0..chunk.e_band.len() {
                let (band, pos, state) = (chunk.e_band[k], chunk.e_pos[k], chunk.e_state[k]);
                if hide == Some((band, pos)) {
                    continue;
                }
                let x = pos & POS_MASK;
                let y = (pos >> POS_BITS) & POS_MASK;
                let z = (pos >> (2 * POS_BITS)) & POS_MASK;
                let level = (pos >> (3 * POS_BITS)) as i32 + MIN_LEVEL;
                let lx = x as f64 * STEP_UNITS;
                let ly = y as f64 * STEP_UNITS;
                let lz = band as f64 * BAND_UNITS + z as f64 * STEP_UNITS;
                let size = (2.0f64).powi(level);
                if !self.paint_entry(volumes, pools, volume, lx, ly, lz, size, state, &mut any) {
                    return Paint { any, refused: true };
                }
            }
        }
        if let Some(c) = carve {
            if !self.paint_entry(volumes, pools, volume, c.lx, c.ly, c.lz, c.size, AIR, &mut any) {
                return Paint { any, refused: true };
            }
        }
        Paint { any, refused: false }
    }

    /// One entry at a corner relative to the column's base. False when a
    /// paint was refused.
    #[allow(clippy::too_many_arguments)]
    fn paint_entry(&mut self, volumes: &mut Volumes, pools: &mut [Pool], volume: usize, lx: f64, ly: f64, lz: f64, size: f64, state: u32, any: &mut bool) -> bool {
        let kind = self.kind(state);
        if kind == Kind::Decor || kind == Kind::Untaught {
            return true;
        }
        let is_sub_cell = size < U_PER_CELL;
        let cell_of = |v: f64| if is_sub_cell { (v / U_PER_CELL).floor() } else { js_round(v / U_PER_CELL) };
        let cx0 = cell_of(lx).max(0.0) as u32;
        let cy0 = cell_of(ly).max(0.0) as u32;
        let z = cell_of(lz) as i32;
        if cx0 >= BAND_CELLS || cy0 >= BAND_CELLS {
            return true;
        }
        let sz_cells = (js_round(size / U_PER_CELL) as u32).max(1);
        let cx1 = (cx0 + sz_cells).min(BAND_CELLS);
        let cy1 = (cy0 + sz_cells).min(BAND_CELLS);
        let is_air = kind == Kind::Air;
        let (material, layer_idx) = if is_air {
            (0, NO_LAYER as u8)
        } else {
            (self.materials[state as usize], self.layers[state as usize])
        };

        let fraction = |v: f64| if is_sub_cell { pack_sub_cell_fraction(v) } else { 0 };
        let sub_sz = if is_sub_cell { pack_sub_cell_fraction(size / U_PER_CELL) } else { SUB_CELL_NONE as u32 };
        let sub_ox = fraction((lx - cx0 as f64 * U_PER_CELL) / U_PER_CELL);
        let sub_oy = fraction((ly - cy0 as f64 * U_PER_CELL) / U_PER_CELL);
        let sub_oz = fraction((lz - z as f64 * U_PER_CELL) / U_PER_CELL);
        let sub = sub_ox | (sub_oy << 8) | (sub_oz << 16) | (sub_sz << 24);
        let v = Voxel { material, layer_idx, sub };

        for cy in cy0..cy1 {
            for cx in cx0..cx1 {
                let address = voxel_address(cx, cy, z);
                *any = true;
                let at = Cell { cx, cy, z };
                let ok = if self.claimed.contains(&address) && !is_air {
                    volumes.append_sub_voxel(pools, volume, at, v)
                } else {
                    self.claimed.insert(address);
                    volumes.set_voxel(pools, volume, at, v)
                };
                if !ok {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(store: &Store, key: u64) -> Vec<(i32, u32, u32)> {
        let Some(lanes) = store.chunk_lanes(key) else { return Vec::new() };
        // SAFETY: the lanes are the chunk's own, read before any change.
        unsafe {
            let n = lanes.count as usize;
            let b = core::slice::from_raw_parts(lanes.band_at, n);
            let p = core::slice::from_raw_parts(lanes.pos_at, n);
            let s = core::slice::from_raw_parts(lanes.state_at, n);
            (0..n).map(|k| (b[k], p[k], s[k])).collect()
        }
    }

    #[test]
    fn a_write_is_read_back_and_derived_as_one_entry() {
        let mut store = Store::new();
        store.write(7, 0, [8, 16, 24], CELL_LEVEL, 3);
        assert_eq!(store.read(7, 0, [8, 16, 24]), 3);
        assert_eq!(store.read(7, 0, [15, 23, 31]), 3, "a cell leaf covers its eight steps");
        assert_eq!(store.read(7, 0, [16, 16, 24]), VOID);
        store.reindex(7);
        assert_eq!(entries(&store, 7), vec![(0, pack_pos(8, 16, 24, CELL_LEVEL), 3)]);
        let cell = store.cell(7, 1, 2, 3).expect("the cell is indexed");
        assert_eq!((cell.start, cell.count, cell.solid), (0, 1, 1));
        let (_, n) = store.take_touched();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_void_write_prunes_the_band_and_the_chunk() {
        let mut store = Store::new();
        store.write(7, 2, [0, 0, 0], MIN_LEVEL, AIR);
        store.reindex(7);
        assert_eq!(store.chunk_count(), 1);
        let cell = store.cell(7, 0, 0, 2 * BAND_CELLS as i32).expect("the cell is indexed");
        assert_eq!(cell.solid, 0, "AIR does not occupy");
        store.write(7, 2, [0, 0, 0], MIN_LEVEL, VOID);
        store.reindex(7);
        assert_eq!((store.chunk_count(), store.band_count(), store.cell_count()), (0, 0, 0));
        assert_eq!(entries(&store, 7), vec![]);
        let (at, n) = store.take_touched();
        assert_eq!(n, 1, "the closed chunk's cell is reported once");
        // SAFETY: the rows were just written and stay until the next change.
        let row = unsafe { core::slice::from_raw_parts(at, 3) };
        assert_eq!(row, &[0, 0, 2 * BAND_CELLS as i32]);
    }

    #[test]
    fn eight_cells_of_one_colour_merge_and_come_back_as_eight_entries() {
        let mut store = Store::new();
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    store.write(1, -1, [x * 8, y * 8, z * 8], CELL_LEVEL, 5);
                }
            }
        }
        store.reindex(1);
        let found = entries(&store, 1);
        assert_eq!(found.len(), 8);
        assert!(found.iter().all(|&(b, p, s)| b == -1 && s == 5 && (p >> (3 * POS_BITS)) == (CELL_LEVEL - MIN_LEVEL) as u32));
        assert_eq!(store.cell_count(), 8);
        assert_eq!(store.cell(1, 1, 1, -(BAND_CELLS as i32) + 1).map(|c| c.count), Some(1));
    }

    #[test]
    fn closing_a_chunk_moves_the_last_into_its_place() {
        let mut store = Store::new();
        store.write(10, 0, [0, 0, 0], CELL_LEVEL, 1);
        store.write(20, 0, [0, 0, 0], CELL_LEVEL, 1);
        store.write(30, 0, [0, 0, 0], CELL_LEVEL, 1);
        store.write(10, 0, [0, 0, 0], CELL_LEVEL, VOID);
        assert_eq!((store.chunk_key(0), store.chunk_key(1)), (30, 20));
        assert_eq!(store.read(30, 0, [0, 0, 0]), 1);
        assert_eq!(store.read(20, 0, [0, 0, 0]), 1);
    }
}
