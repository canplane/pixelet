// pxl-core/domain/view/above_ground.rs

//! Where the ground under a point stands: a point's height over it.

use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use crate::domain::query::World;
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

/// From the point's own height, so a cave's floor counts in a cave; the
/// datum where nothing is held.
fn ground_under(at: V3) -> f64 {
    let point = world::point(at[0], at[1], at[2]);
    World::now()
        .floor_under(&point, LEAF_ZOOM)
        .val()
        .flatten()
        .unwrap_or(0.0)
}

/// What the origin grain is measured from.
pub fn height_at(at: V3) -> f64 {
    (at[2] - ground_under(at)).max(0.0)
}
