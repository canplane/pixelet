// pxl-spec/bake/constant.rs

use crate::space::coord::zoom::{Zoom, EXTENT, FINEST_ZOOM, MIN_ZOOM, UNIT_ZOOM};
use crate::space::layer::lattice::GROUND;

// Elevation is sampled at cell centers (x + 0.5, y + 0.5) by both the SQL
// resample and the bake's bicubic, and stored as i16 ground cells.
const _: () = assert!(
    EXTENT[2] == i16::BITS && GROUND.leaf_cell_zoom() == UNIT_ZOOM,
    "an elevation in i16 ground cells spans the world's height"
);
pub use super::object::OBJECT_SCALE_CNT;

/// The objects' coarsest zoom, at scale 0: a placed cell of one unit. The scales are the
/// objects' own and never the ground's cell.
pub const OBJECT_COARSEST: Zoom = UNIT_ZOOM;

pub const OBJECT_FINEST: Zoom = OBJECT_COARSEST + OBJECT_SCALE_CNT as Zoom - 1;
const _: () = assert!(
    OBJECT_FINEST < FINEST_ZOOM && GROUND.grid_zoom(MIN_ZOOM) >= 1,
    "the zooms a cell is asked at, and so every node a ray stops on, lie from 1 to the finest's coarser, where a slot may keep it (pxl-engine node_cache::keeps)"
);
