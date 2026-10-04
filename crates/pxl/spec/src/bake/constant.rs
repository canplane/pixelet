// pxl-spec/bake/constant.rs

use crate::space::hierarchy::{ancestor_of, first_descendant_of, Scale};
use crate::space::indexer::{L2, L3};
use crate::space::pvf3::ivec::IVec;
use crate::space::sparse_tree::mem::SparseTreeLayout;

/// The raster cell's side against the unit. It is a root of its own beside
/// the tiers: changing it rescales page and chunk cell counts, the object
/// ladder and the band's cells together.
pub const CELL: Scale = Scale::of(0);

pub const U_PER_CELL: f64 = CELL.side();

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the client's bicubic, and stored as i16 cells.
/// Inner elevation samples across one L3 page: a page is about 2^(L3 + 1) m
/// wide and the source is about 30 m, rounded to 32 m.
pub const PAGE_ELE_SPAN: u32 = 1 << (L3.shamt() + 1 - 5);

/// Two samples of padding give the bicubic a 4×4 kernel at every edge texel,
/// and let the ground height 32 cells past the page be read without the
/// neighboring page resident.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

/// Sample (x, y) of a padded elevation grid, rows from the south.
pub const fn ele_index(x: u32, y: u32) -> usize {
    (y * ELE_PADDED_SPAN + x) as usize
}

/// Cells on a page's side in its index lane (bake/format.rs `Index`): one per
/// elevation sample spacing, at 4 KiB a page.
pub const EXTENT_SPAN: u32 = PAGE_ELE_SPAN;

pub const EXTENT_CELL: Scale = L3.down(EXTENT_SPAN.trailing_zeros());

const _: () = assert!(EXTENT_SPAN.is_power_of_two(), "an index cell is a block of the lattice, a power of two of the unit");

// ---- The deck lane ----

/// A deck cell's side against the unit (bake/deck.rs): two of the ground's
/// cells. A deck is a way carried over the ground — a bridge, a viaduct — and
/// a page holds it in a lane of its own, which every resident record carries
/// whether or not anything crosses the page: at the ground's resolution that
/// lane is half the ground lane again, some 30 MiB across the records, and at
/// two cells a side it is an eighth of that. Two cells is about 4 m at the
/// equator, which a carriageway, a sidewalk or a track is no narrower than,
/// so what the coarser lane loses is where a deck's edge falls inside a
/// cell, not whether it is there.
pub const DECK_CELL: Scale = CELL.up(1);

/// How many sizes an object places at: a cell and its halvings, each tier a
/// level of the object layout below.
///
/// 4 deliberately, not 3. When the cell changed U_PER_CELL from 0.5u to 1u,
/// sliding a fixed count of tiers down from it would have silently dropped
/// the finest one (0.125u) that building at sub-block detail already
/// depended on, instead of gaining a coarser option to match the new cell
/// size. One more halving keeps every size that already worked.
pub const OBJECT_TIERS: usize = 4;

pub const OBJECT_FINEST: Scale = CELL.down(OBJECT_TIERS as u32 - 1);

/// A band: a column's chunk stood up as a cube, so one octree each.
pub const BAND: Scale = L2;

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

/// An object entry's address: its column (the L2 index of its XY), band and packed position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryAt {
    pub col_idx: IVec,
    pub band_idx: BandZ,
    pub pos: u32,
}

/// A band-local column is `(cy << BAND_CELL_SHAMT) | cx`.
pub const BAND_CELLS: u32 = CELL.count_in(BAND);
pub const BAND_CELL_SHAMT: u32 = BAND.above(CELL) as u32;
pub const CELL_STEPS: u32 = OBJECT_FINEST.count_in(CELL);
pub const STEP_UNITS: f64 = OBJECT_FINEST.side();
pub const BAND_UNITS: f64 = BAND.side();
pub const POS_BITS: u32 = BAND.above(OBJECT_FINEST) as u32;
pub const POS_MASK: u32 = (1 << POS_BITS) - 1;

/// An entry's packed position: x, y, z in finest-tier steps from its band's
/// corner, then its tier above the finest.
pub const fn pack_pos(x: u32, y: u32, z: u32, scale: Scale) -> u32 {
    x | (y << POS_BITS) | (z << (2 * POS_BITS)) | ((scale.above(OBJECT_FINEST) as u32) << (3 * POS_BITS))
}

pub const fn unpack_pos(pos: u32) -> (u32, u32, u32, Scale) {
    (
        pos & POS_MASK,
        (pos >> POS_BITS) & POS_MASK,
        (pos >> (2 * POS_BITS)) & POS_MASK,
        OBJECT_FINEST.up((pos >> (3 * POS_BITS)) as i32),
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
    let levels = CELL.above(OBJECT_FINEST) as u32;
    let cell = ancestor_of(step.map(i64::from), levels);
    let base = first_descendant_of(cell, levels);
    (cell.map(|v| v as u32), std::array::from_fn(|i| (i64::from(step[i]) - base[i]) as u32))
}
