// pxl-core/domain/control/edit/pick.rs

//! The pointer's pick, and what a hit means for a placement or an erase.
//! The pick asks the world at terrain's finest zoom, the leaves', because
//! the leaves are what the screen shows around the eye.

use crate::domain::layer::instance::{Instance, Source};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{units, Zoom};

use crate::domain::layer::object::store::Entry;
use crate::domain::query::{finest, Hit, HitKind, Ray, World};
use crate::domain::view::frame::coord::world_to_canonical;
use pxl_spec::space::coord::aabb::Aabb;

/// How far from the eye the pointer can pick, in units. It is a product
/// decision: what can be edited is a sphere about the observer.
const PICK_DIST_U: f64 = 32.0;

// Larger than the error of a round trip through the 1/256 grid (±1/512 u),
// and smaller than the gap between any two object tiers (the finest tier's
// side).
const SAME_EXTENT_EPS: f64 = units(OBJECT_FINEST) / 16.0;

/// The octree stores an object of side `size` only on these grid lines. A
/// value within `SAME_EXTENT_EPS` of a line counts as on it.
fn grid_ceil(v: f64, size: f64) -> f64 {
    ((v - SAME_EXTENT_EPS) / size).ceil() * size
}

fn grid_floor(v: f64, size: f64) -> f64 {
    ((v + SAME_EXTENT_EPS) / size).floor() * size
}

/// The corner a new cube of `zoom` takes flush against the face `hit`
/// struck. On the axis of the face's normal, the cube starts just past the
/// struck box. On the other axes, it takes the cell the hit point falls in.
/// `precise_box` replaces the hit's box where that is not exact, as for a
/// placed cube's stored entry.
pub fn world_adjacent_corner(hit: &Hit, zoom: Zoom, precise_box: Option<&Aabb>) -> Q3 {
    let new_size = units(zoom);
    let Aabb {
        min: box_min,
        max: box_max,
    } = precise_box.copied().unwrap_or(hit.cube);
    let mut out: V3 = [0.0; 3];

    for axis in 0..3 {
        let n = hit.normal[axis];
        out[axis] = if n > 0.0 {
            grid_ceil(box_max[axis], new_size)
        } else if n < 0.0 {
            grid_floor(box_min[axis], new_size) - new_size
        } else {
            grid_floor(hit.point[axis], new_size)
        };
    }

    world_to_canonical(&out)
}

/// The corner of the cell of `zoom` that `hit` struck, for the eraser's dig
/// into terrain. It follows `world_adjacent_corner` but stays inside the
/// struck box on the normal's axis.
#[expect(
    clippy::needless_range_loop,
    reason = "an axis indexes the hit's normal, its box and the corner at once"
)]
pub fn world_struck_corner(hit: &Hit, zoom: Zoom) -> Q3 {
    let size = units(zoom);
    let mut out: V3 = [0.0; 3];
    for axis in 0..3 {
        let n = hit.normal[axis];
        out[axis] = if n > 0.0 {
            grid_ceil(hit.cube.max[axis], size) - size
        } else if n < 0.0 {
            grid_floor(hit.cube.min[axis], size)
        } else {
            grid_floor(hit.point[axis], size)
        };
    }
    world_to_canonical(&out)
}

/// The placed object struck: a cube, or a decor someone placed. None for
/// the ground and for decor the ground generated.
pub fn resolve_hit_object(hit: &Hit) -> Option<Entry> {
    match hit.kind {
        HitKind::Object(at)
        | HitKind::Decor(Instance {
            src: Source::Object(at),
            ..
        }) => World::now().entry_at(at),
        HitKind::Ground | HitKind::Objects | HitKind::Decor(_) => None,
    }
}

/// The first thing the ray from the eye at `origin` along `dir` strikes
/// within `PICK_DIST_U`, in the root frame. Decor is struck where the frame
/// holds it. The world asked is the stored one (`World::now`), not the one
/// the eraser's preview draws, because what is erased is what is stored.
pub fn ray_pick(origin: V3, dir: V3) -> Option<Hit> {
    let ray = Ray {
        origin,
        dir,
        t_max: PICK_DIST_U,
    };
    // Only an exact answer counts. A coarser tile's answer is a summary, not
    // what is held.
    World::now()
        .striking_decor()
        .first_hit(&ray, finest())
        .exact()
        .flatten()
}

#[cfg(test)]
#[path = "pick.test.rs"]
mod test;
