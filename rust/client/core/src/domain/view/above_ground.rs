// pxl-core/domain/view/above_ground.rs

//! A point's height over the ground under it.

use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;

use crate::domain::query::World;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;

/// The height of the floor under `at`. The search starts at the point's own
/// height, so a point inside a cave finds the cave's floor and not the
/// ground above it. Where the world holds nothing, the floor is the datum, 0.
fn ground_under(at: V3) -> f64 {
    let point = world::point(at[0], at[1], at[2]);
    World::now()
        .floor_under(&point, LEAF_ZOOM)
        .val()
        .flatten()
        .unwrap_or(0.0)
}

pub(crate) fn height_at(at: V3) -> f64 {
    (at[2] - ground_under(at)).max(0.0)
}
