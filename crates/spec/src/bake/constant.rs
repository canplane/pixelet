// pxl-spec/bake/constant.rs

use crate::space::coord::world::{wrap_cell, EXTENT};
use crate::space::coord::zoom::{Zoom, UNIT_ZOOM};
use crate::space::layer::lattice::GROUND;

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the client's bicubic, and stored as i16 ground cells.
const _: () = assert!(EXTENT[2] == i16::BITS && GROUND.cell() == UNIT_ZOOM, "a height in i16 ground cells is the world's height");
/// A tile's elevation samples and their padding, one table with the GPU's
/// (pxl-portable).
pub use pxl_portable::ground::{ele_index, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};

/// The index lane's side and its cells' zoom, one table with the GPU's
/// (pxl-portable).
pub use pxl_portable::ground::{EXTENT_CELL, EXTENT_SPAN};

/// A GPU's grid of a held tile, its cells' heights then their bound's
/// levels (pxl-portable).
pub use pxl_portable::ground::{BOUND_LEVELS, GRID_WORDS};

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
