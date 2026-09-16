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

use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::pvf3::{ivec, Ivec};
use pxl_shared::spec::space::sparse_tree::mem;
use pxl_shared::spec::space::sparse_tree::tree::{self as sparse_tree, SparseTree, SparseTreeLayout};

use crate::bake::terrain::js_round;
use crate::bake::volume::{Cell, Volumes, Voxel};
use crate::bake::voxel::{pack_sub_cell_fraction, NO_LAYER, SUB_CELL_NONE};
use crate::space::constant::U_PER_CELL;

/// A cell's Z and a band's, as wide as the coordinate carries them. PVF3
/// gives Z sixteen bits (pvf3/core.rs's IVEC_Z_W), so a cell's Z is an i16 and
/// a band's — that Z over BAND_CELLS — is narrower still. They are i32 here
/// because the arithmetic over them (a band times its cells) leaves the range
/// a Z has, and a type that cannot hold its own product is worse than a wide
/// one: what the width is for is written down rather than enforced.
pub type CellZ = i32;
pub type BandZ = i32;

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
fn band_key(at: [u32; 3]) -> IVec {
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
    /// The column this chunk is, as the key everything about it is filed under.
    col_idx: IVec,
    /// Index → state; index 0 is VOID in every chunk.
    palette: Vec<u32>,
    /// The band Z range the chunk's bands lie in; a band inside it may be gone.
    band_min: BandZ,
    band_max: BandZ,
    /// The entries, derived by `reindex`.
    e_band: Vec<BandZ>,
    e_pos: Vec<u32>,
    e_state: Vec<u32>,
}

/// A cell by its column and its absolute cell Z, and its band-local column.
type CellKey = (IVec, CellZ, u32);

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
    pub band_at: *const BandZ,
    pub pos_at: *const u32,
    pub state_at: *const u32,
}

pub struct Store {
    /// Chunks in the order they were opened, the last moved into a closed
    /// one's place — which is the order the persisted form lists them in.
    chunks: Vec<Chunk>,
    chunk_at: BTreeMap<IVec, u32>,
    bands: BTreeMap<(IVec, BandZ), SparseTree<u16>>,
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

    pub fn chunk_key(&self, i: usize) -> IVec {
        self.chunks[i].col_idx
    }

    pub fn band_count(&self) -> usize {
        self.bands.len()
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    fn open_chunk(&mut self, col_idx: IVec) -> usize {
        if let Some(&pos) = self.chunk_at.get(&col_idx) {
            return pos as usize;
        }
        let pos = self.chunks.len();
        self.chunks.push(Chunk {
            col_idx,
            palette: vec![VOID],
            band_min: i32::MAX,
            band_max: i32::MIN,
            e_band: Vec::new(),
            e_pos: Vec::new(),
            e_state: Vec::new(),
        });
        self.chunk_at.insert(col_idx, pos as u32);
        pos
    }

    /// Removes a chunk that has no bands left, moving the last one into its
    /// position.
    fn close_chunk(&mut self, pos: usize) {
        self.unindex_cells(pos);
        let gone = self.chunks.swap_remove(pos);
        self.chunk_at.remove(&gone.col_idx);
        if pos < self.chunks.len() {
            self.chunk_at.insert(self.chunks[pos].col_idx, pos as u32);
        }
    }

    fn open_band(&mut self, cpos: usize, band_idx: BandZ) {
        let key = self.chunks[cpos].col_idx;
        if self.bands.contains_key(&(key, band_idx)) {
            return;
        }
        self.bands.insert((key, band_idx), sparse_tree::alloc::<u16>(BAND_LAYOUT, 64, None));
        let chunk = &mut self.chunks[cpos];
        chunk.band_min = chunk.band_min.min(band_idx);
        chunk.band_max = chunk.band_max.max(band_idx);
    }

    /// Removes a band, then tightens its chunk's Z range — and removes the
    /// chunk when nothing is left in it.
    fn close_band(&mut self, cpos: usize, band_idx: BandZ) {
        let key = self.chunks[cpos].col_idx;
        if let Some(mut tree) = self.bands.remove(&(key, band_idx)) {
            sparse_tree::free(&mut tree);
        }
        let chunk = &self.chunks[cpos];
        let mut left = self.bands.range((key, chunk.band_min)..=(key, chunk.band_max)).map(|(&(_, bz), _)| bz);
        let lo = left.next().unwrap_or(BandZ::MAX);
        let hi = left.next_back().unwrap_or(lo);
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
    pub fn read(&self, col_idx: IVec, band_idx: BandZ, at: [u32; 3]) -> u32 {
        let Some(&cpos) = self.chunk_at.get(&col_idx) else { return VOID };
        let Some(tree) = self.bands.get(&(col_idx, band_idx)) else { return VOID };
        self.chunks[cpos as usize].palette[mem::get::<Ivec, u16>(&tree.mem, &tree.layout, band_key(at), 0) as usize]
    }

    /// Writes `state` as one leaf of `level` at a lattice position of band
    /// `z` of chunk `key`, compacts the band, and drops the band — and the
    /// chunk — once every leaf reads VOID. The entries are not re-derived:
    /// that is `reindex`, once, after however many writes.
    pub fn write(&mut self, col_idx: IVec, band_idx: BandZ, at: [u32; 3], level: i32, state: u32) {
        assert!((MIN_LEVEL..=CELL_LEVEL).contains(&level), "an object level outside its layout");
        let cpos = match self.chunk_at.get(&col_idx) {
            Some(&pos) => pos as usize,
            None if state == VOID => return,
            None => self.open_chunk(col_idx),
        };
        if !self.bands.contains_key(&(col_idx, band_idx)) {
            if state == VOID {
                return;
            }
            self.open_band(cpos, band_idx);
        }
        let idx = self.palette_index(cpos, state);
        let tree = self.bands.get_mut(&(col_idx, band_idx)).expect("the band just opened");
        mem::set::<Ivec, u16>(&mut tree.mem, &tree.layout, band_key(at), idx, level, 0);
        sparse_tree::compact(tree);
        if band_is_empty(tree) {
            self.close_band(cpos, band_idx);
        }
    }

    /// Re-derives one chunk's entries and cells; a chunk that is gone has
    /// nothing to derive, and its cells were unindexed as it closed.
    pub fn reindex(&mut self, col_idx: IVec) {
        let Some(&cpos) = self.chunk_at.get(&col_idx) else { return };
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
        // One descent to the column's first band, then its bands in order: the
        // key is the column before the Z, so a column's bands lie together and
        // the Z between two of them is a step rather than a search.
        for (&(_, bz), tree) in self.bands.range((chunk.col_idx, chunk.band_min)..=(chunk.col_idx, chunk.band_max)) {
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

    fn cell_of(col_idx: IVec, band_idx: BandZ, pos: u32) -> CellKey {
        let (x, y, z, _) = unpack_pos(pos);
        (col_idx, band_idx * BAND_CELLS as i32 + (z / CELL_STEPS) as i32, ((y / CELL_STEPS) << BAND_SHAMT) | (x / CELL_STEPS))
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
            let cell = Self::cell_of(chunk.col_idx, chunk.e_band[k], chunk.e_pos[k]);
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
            let cell = Self::cell_of(chunk.col_idx, chunk.e_band[k], chunk.e_pos[k]);
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
    pub fn chunk_lanes(&self, col_idx: IVec) -> Option<ChunkLanes> {
        let &cpos = self.chunk_at.get(&col_idx)?;
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
    pub fn cell(&self, col_idx: IVec, cx: u32, cy: u32, cz: CellZ) -> Option<CellRec> {
        self.cells.get(&(col_idx, cz, (cy << BAND_SHAMT) | cx)).copied()
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

/// What a cell's Z is shifted by to address it unsigned: PVF3 gives Z sixteen
/// bits, so this is half that range and the sum is one.
const Z_BIAS: CellZ = 1 << 15;

/// A voxel's address within a paint: its Z above the bias, then its cell in
/// the band. Sixteen bits of Z over twelve of column is twenty-eight, which is
/// what the claimed set is keyed by.
fn voxel_address(cx: u32, cy: u32, z: CellZ) -> u32 {
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
    pub fn paint(&mut self, store: &Store, volumes: &mut Volumes, volume: usize, col_idx: IVec, hide: Option<(i32, u32)>, carve: Option<Carve>) -> Paint {
        self.claimed.clear();
        let mut any = false;
        if let Some(&cpos) = store.chunk_at.get(&col_idx) {
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
                if !self.paint_entry(volumes, volume, lx, ly, lz, size, state, &mut any) {
                    return Paint { any, refused: true };
                }
            }
        }
        if let Some(c) = carve {
            if !self.paint_entry(volumes, volume, c.lx, c.ly, c.lz, c.size, AIR, &mut any) {
                return Paint { any, refused: true };
            }
        }
        Paint { any, refused: false }
    }

    /// One entry at a corner relative to the column's base. False when a
    /// paint was refused.
    #[allow(clippy::too_many_arguments)]
    fn paint_entry(&mut self, volumes: &mut Volumes, volume: usize, lx: f64, ly: f64, lz: f64, size: f64, state: u32, any: &mut bool) -> bool {
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
                    volumes.append_sub_voxel(volume, at, v)
                } else {
                    self.claimed.insert(address);
                    volumes.set_voxel(volume, at, v)
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
#[path = "object.test.rs"]
mod test;
