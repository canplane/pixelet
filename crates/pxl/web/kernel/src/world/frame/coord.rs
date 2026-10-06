// pxl-kernel/world/frame/coord.rs

//! The world in doubles: a double holds the whole lattice with its fraction
//! exactly, so positions are kept as the world's, X running on past a lap of
//! it so that it is continuous about the eye.

use pxl_spec::space::pvf3::nearest;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::round;

use crate::world::frame::view::view;

/// A canonical point in the world's doubles, on the lap nearest `near_x`.
pub fn canonical_to_world(p: &Pvf3, near_x: f64) -> V3 {
    let [x, y, z] = projection::unproject(p.i, p.f);
    [nearest(x, near_x), y, z]
}

/// A canonical point in the world's doubles, on the lap nearest the eye.
pub fn to_world(p: &Pvf3) -> V3 {
    canonical_to_world(p, view().pose.eye[0])
}

/// Through `project`, the one door into PVF3: X goes round, Y and Z stop at
/// the edge.
pub fn world_to_canonical(w: &V3) -> Pvf3 {
    projection::project(w[0], w[1], w[2])
}

/// The nearest canonical point to `w`, unlike `world_to_canonical`, which
/// takes the cell it lies in. A point that went through floats and came back
/// a hair under would otherwise step down each round trip.
pub fn world_to_canonical_nearest(w: &V3) -> Pvf3 {
    let one = pxl_spec::space::pvf3::core::ONE as f64;
    world_to_canonical(&w.map(|v| round(v * one) / one))
}

#[cfg(test)]
#[path = "coord.test.rs"]
mod test;
