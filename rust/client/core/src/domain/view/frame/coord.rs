// pxl-core/domain/view/frame/coord.rs

//! Positions in the world's doubles. A double holds every lattice point with
//! its fraction exactly, so the view keeps positions as world coordinates.
//! X is not wrapped into one lap of the world: it runs on past the seam, so
//! positions stay continuous around the eye.

use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::nearest;

use crate::domain::view::frame::pose::view;

fn canonical_to_world(p: &Q3, near_x: f64) -> V3 {
    let [x, y, z] = q3::unpack(p);
    [nearest(x, near_x), y, z]
}

/// The canonical point `p` in the world's doubles, on the lap of X nearest
/// the eye.
pub fn to_world(p: &Q3) -> V3 {
    canonical_to_world(p, view().at().eye[0])
}

/// The canonical point of the lattice cell `w` lies in. It goes through
/// `world::point`, the only way a position enters the world: X wraps around
/// the world, and Y and Z are held at its edge.
pub fn world_to_canonical(w: &V3) -> Q3 {
    world::point(w[0], w[1], w[2])
}

#[cfg(test)]
#[path = "coord.test.rs"]
mod test;
