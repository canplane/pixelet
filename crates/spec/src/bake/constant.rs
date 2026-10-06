// pxl-spec/bake/constant.rs

use crate::space::coord::pvf3::core::IVEC_Z_W;
use crate::space::coord::world::wrap_cell;
use crate::space::coord::zoom::{shamt, Zoom, UNIT_ZOOM};
use crate::space::layer::lattice::GROUND;
use crate::space::layer::tile::LEAF_ZOOM;

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the client's bicubic, and stored as i16 ground cells.
const _: () = assert!(IVEC_Z_W == i16::BITS && GROUND.cell() == UNIT_ZOOM, "a height in i16 ground cells is the Z lane");
/// Inner elevation samples across one leaf tile: a tile is about twice its
/// side in units in meters wide and the source is about 30 m, rounded to 32 m.
pub const TILE_ELE_SPAN: u32 = 1 << (shamt(LEAF_ZOOM) + 1 - 5);

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

pub const EXTENT_CELL: Zoom = LEAF_ZOOM + EXTENT_SPAN.trailing_zeros() as Zoom;

const _: () = assert!(EXTENT_SPAN.is_power_of_two(), "an index cell is a block of the lattice, a power of two of the unit");

/// A deck cell: two ground cells, about 4 m at the equator, narrower than no
/// way; at the ground's resolution the lane would cost some 30 MiB of records.
pub const DECK_CELL: Zoom = GROUND.cell() - 1;

pub use super::object::OBJECT_TIERS;

/// The objects' coarsest tier, a placed block, one unit: the ladder is the
/// objects' own and never the ground's cell.
pub const OBJECT_COARSEST: Zoom = UNIT_ZOOM;

pub const OBJECT_FINEST: Zoom = OBJECT_COARSEST + OBJECT_TIERS as Zoom - 1;

/// A placed object's address: its cube at its own zoom, named as
/// `wrap_cell` names it, so one cube has one name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EntryAt {
    zoom: Zoom,
    at: [i64; 3],
}

impl EntryAt {
    pub fn new(zoom: Zoom, at: [i64; 3]) -> EntryAt {
        EntryAt { zoom, at: wrap_cell(zoom, at) }
    }

    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    pub fn at(self) -> [i64; 3] {
        self.at
    }
}
