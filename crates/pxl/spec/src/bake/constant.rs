// pxl-spec/bake/constant.rs

use crate::space::hierarchy::{ancestor_of, first_descendant_of};
use crate::space::pvf3::ivec::IVec;
use crate::space::pyramid::{CHUNK_STEPS, LEAF_Z};
use crate::space::sparse_tree::mem::SparseTreeLayout;
use crate::space::zoom::{count, shamt, units, Zoom, UNIT_Z};

/// The raster cell's zoom. It is a root of its own: changing it rescales
/// the object ladder and the band's cells together.
pub const CELL: Zoom = UNIT_Z;

pub const U_PER_CELL: f64 = units(CELL);

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the client's bicubic, and stored as i16 cells.
/// Inner elevation samples across one leaf tile: a tile is about twice its
/// side in units in meters wide and the source is about 30 m, rounded to 32 m.
pub const TILE_ELE_SPAN: u32 = 1 << (shamt(LEAF_Z) + 1 - 5);

/// Two samples of padding give the bicubic a 4×4 kernel at every edge texel,
/// and let the ground height 32 cells past the tile be read without the
/// neighboring tile resident.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = TILE_ELE_SPAN + 2 * ELE_PAD;

/// Sample (x, y) of a padded elevation grid, rows from the south.
pub const fn ele_index(x: u32, y: u32) -> usize {
    (y * ELE_PADDED_SPAN + x) as usize
}

/// Cells on a tile's side in its index lane (bake/format.rs `Index`): one per
/// elevation sample spacing, at 4 KiB a tile.
pub const EXTENT_SPAN: u32 = TILE_ELE_SPAN;

pub const EXTENT_CELL: Zoom = LEAF_Z + EXTENT_SPAN.trailing_zeros() as Zoom;

const _: () = assert!(EXTENT_SPAN.is_power_of_two(), "an index cell is a block of the lattice, a power of two of the unit");

/// A deck cell: two ground cells, about 4 m at the equator, narrower than no
/// way; at the ground's resolution the lane would cost some 30 MiB of records.
pub const DECK_CELL: Zoom = CELL - 1;

pub use super::object::OBJECT_TIERS;

pub const OBJECT_FINEST: Zoom = CELL + OBJECT_TIERS as Zoom - 1;

/// A band: a column's chunk on a leaf stood up as a cube, so one octree each.
pub const BAND: Zoom = CELL - CHUNK_STEPS;

/// A band's octree, from the finest tier to the band. A uniform block folds up
/// to the root, so a leaf may stand for a whole band.
pub const OBJECT_LAYOUT: SparseTreeLayout = SparseTreeLayout {
    dim: 3,
    min: OBJECT_FINEST,
    max: BAND,
};

/// Z widths: PVF3 gives Z 16 bits, but products of them leave that range, so they are i32.
pub type CellZ = i32;
pub type BandZ = i32;

/// An object entry's address: its column (the band-zoom index of its XY), band and packed position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryAt {
    pub col_idx: IVec,
    pub band_idx: BandZ,
    pub pos: u32,
}

/// A band-local column is `(cy << BAND_CELL_SHAMT) | cx`.
pub const BAND_CELLS: u32 = count(CELL, BAND);
pub const BAND_CELL_SHAMT: u32 = (CELL - BAND) as u32;
pub const CELL_STEPS: u32 = count(OBJECT_FINEST, CELL);
pub const STEP_UNITS: f64 = units(OBJECT_FINEST);
pub const BAND_UNITS: f64 = units(BAND);
pub const POS_BITS: u32 = (OBJECT_FINEST - BAND) as u32;
pub const POS_MASK: u32 = (1 << POS_BITS) - 1;

/// An entry's packed position: x, y, z in finest-tier steps from its band's
/// corner, then its tier above the finest.
pub const fn pack_pos(x: u32, y: u32, z: u32, cell_zoom: Zoom) -> u32 {
    x | (y << POS_BITS) | (z << (2 * POS_BITS)) | (((OBJECT_FINEST - cell_zoom) as u32) << (3 * POS_BITS))
}

pub const fn unpack_pos(pos: u32) -> (u32, u32, u32, Zoom) {
    (
        pos & POS_MASK,
        (pos >> POS_BITS) & POS_MASK,
        (pos >> (2 * POS_BITS)) & POS_MASK,
        OBJECT_FINEST - (pos >> (3 * POS_BITS)) as Zoom,
    )
}

const _: () = assert!(3 * POS_BITS < 32 && OBJECT_TIERS <= 1 << (32 - 3 * POS_BITS), "an entry's tier fits beside its position");

pub fn band_of(cell_z: i64) -> (BandZ, u32) {
    let [band] = ancestor_of([cell_z], BAND_CELL_SHAMT);
    (band as BandZ, (cell_z - band_cell_z(band as BandZ)) as u32)
}

pub fn band_cell_z(band: BandZ) -> i64 {
    first_descendant_of([i64::from(band)], BAND_CELL_SHAMT)[0]
}

/// A band position in finest-tier steps as its band-local cell and the steps into it.
pub fn in_cell(step: [u32; 3]) -> ([u32; 3], [u32; 3]) {
    let steps = (OBJECT_FINEST - CELL) as u32;
    let cell = ancestor_of(step.map(i64::from), steps);
    let base = first_descendant_of(cell, steps);
    (cell.map(|v| v as u32), std::array::from_fn(|i| (i64::from(step[i]) - base[i]) as u32))
}
