// pxl-core/domain/layer/tile_cache/floor.rs

//! The floor: the tiles at the layer's top that are held hard under every
//! observed piece of ground, so that every observation has an answer at some
//! zoom. The floor is asked for with every observation, not only when an
//! observation finds nothing; otherwise each turn toward new ground would
//! leave a hole for a whole round trip.
//!
//! The floor's zoom is the layer's coarsest (`Base::min_zoom`), which the source
//! declares (sys/io/source.rs). Until the source is declared, the floor is
//! empty.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::demand::{demand_tiles, tiles_within};
use pxl_spec::space::layer::lattice::Lattice;
use pxl_spec::space::layer::quadrant::Quadrant;

/// Calls `each` with the tiles at `top`, the coarsest zoom the source
/// supplies whole, whose ground lies within the eye's far edge
/// (`Observation::demand_dist`). These hold every observed piece of ground.
/// An eye higher than its far edge gets none: a pixel there covers more than
/// a cell at the top, and the view has gone over to the Earth's picture.
pub fn floor(o: &Observation, top: Zoom, each: impl FnMut(Quadrant)) {
    if let Some(r) = ring(o) {
        tiles_within(top, [o.eye[0], o.eye[1]], r).for_each(each);
    }
}

/// The most tiles `floor` names in a layer's `lattice`. The far edge reaches
/// no farther than the top's demand, and the ground's cut through a sphere
/// of that radius is no wider, so the floor names no more tiles than one
/// zoom's demand does.
pub const fn tiles(lattice: Lattice) -> u32 {
    demand_tiles(lattice)
}

/// The radius, along the ground, within which the top's ground is held:
/// where the sphere of the eye's far edge cuts the ground. None where the eye
/// is higher than its far edge.
pub fn ring(o: &Observation) -> Option<f64> {
    let over = (o.eye[2] - o.ground).max(0.0);
    (over < o.demand_dist).then(|| (o.demand_dist * o.demand_dist - over * over).sqrt())
}
