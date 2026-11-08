// pxl-spec/bake/constant.rs

use crate::space::coord::world::EXTENT;
use crate::space::coord::zoom::{Zoom, UNIT_ZOOM};
use crate::space::layer::lattice::GROUND;
use crate::space::layer::quadrant::TOP_ZOOM;
use pxl_engine::FINEST_ZOOM;

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the bake's bicubic, and stored as i16 ground cells.
const _: () = assert!(
    EXTENT[2] == i16::BITS && GROUND.leaf_cell_zoom() == UNIT_ZOOM,
    "a height in i16 ground cells is the world's height"
);
/// A leaf's elevation samples and their padding, which the bake draws its
/// cells' heights off (pxl-engine).
pub use pxl_engine::ground::{ele_idx, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};

pub use super::object::OBJECT_SCALES;

/// The objects' coarsest zoom, at scale 0: a placed cell of one unit. The scales are the
/// objects' own and never the ground's cell.
pub const OBJECT_COARSEST: Zoom = UNIT_ZOOM;

pub const OBJECT_FINEST: Zoom = OBJECT_COARSEST + OBJECT_SCALES as Zoom - 1;
const _: () = assert!(
    OBJECT_FINEST < FINEST_ZOOM && GROUND.grid_zoom(TOP_ZOOM) >= 1,
    "the zooms a cell is asked at, and so every node a ray stops on, lie from 1 to the finest's coarser, where a slot may keep it (pxl-engine node_cache::keeps)"
);
