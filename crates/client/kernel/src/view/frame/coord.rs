// pxl-kernel/view/frame/coord.rs

//! The world in doubles: a double holds the whole lattice with its fraction
//! exactly, so positions are kept as the world's, X running on past a lap of
//! it so that it is continuous about the eye.

use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::nearest;
use xpute_kit::math::scalar::round;

use crate::view::frame::pose::view;

/// A canonical point in the world's doubles, on the lap nearest `near_x`.
pub fn canonical_to_world(p: &Q3, near_x: f64) -> V3 {
    let [x, y, z] = q3::unpack(p);
    [nearest(x, near_x), y, z]
}

/// A canonical point in the world's doubles, on the lap nearest the eye.
pub fn to_world(p: &Q3) -> V3 {
    canonical_to_world(p, view().at().eye[0])
}

/// Through `project`, the one door into a position: X goes round, Y and Z stop at
/// the edge.
pub fn world_to_canonical(w: &V3) -> Q3 {
    world::point(w[0], w[1], w[2])
}

/// The nearest canonical point to `w`, unlike `world_to_canonical`, which
/// takes the cell it lies in. A point that went through floats and came back
/// a hair under would otherwise step down each round trip.
pub fn world_to_canonical_nearest(w: &V3) -> Q3 {
    let one = q3::ONE as f64;
    world_to_canonical(&w.map(|v| round(v * one) / one))
}

#[cfg(test)]
#[path = "coord.test.rs"]
mod test;
