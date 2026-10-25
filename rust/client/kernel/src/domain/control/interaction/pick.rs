// pxl-kernel/domain/control/interaction/pick.rs

//! The pointer's pick, asked of the access at terrain's finest zoom, the leaves', which
//! is what is on the screen about the eye; and what a hit means for a
//! placement or an erase.

use crate::domain::layer::instance::{Instance, Source};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;

use crate::domain::layer::object::store::Entry;
use crate::domain::query::{held, Hit, Ray, Struck, World};
use crate::domain::view::frame::coord::world_to_canonical;
use pxl_spec::space::coord::aabb::Aabb;

/// A product number: what can be edited is a sphere about the observer.
const PICK_REACH_U: f64 = 32.0;

// Above the 1/256-grid round trip's error (±1/512 u), below the gap between
// any two object tiers (the finest's side).
const SAME_EXTENT_EPS: f64 = units(OBJECT_FINEST) / 16.0;

/// The octree only stores a `size` object on these lines; any other corner
/// was re-snapped by the write later, which caused overlapping placements.
fn grid_ceil(v: f64, size: f64) -> f64 {
    ((v - SAME_EXTENT_EPS) / size).ceil() * size
}

fn grid_floor(v: f64, size: f64) -> f64 {
    ((v + SAME_EXTENT_EPS) / size).floor() * size
}

/// The corner a new `new_size` cube takes flush against what `hit` struck,
/// one rule per axis by the sign of that axis's normal: n > 0 starts at
/// grid_ceil(max), n < 0 ends at grid_floor(min), n = 0 takes the
/// cell the hit point falls in. `precise_box` overrides the hit's box (a
/// decor met in front of it).
#[allow(clippy::needless_range_loop)]
pub fn world_adjacent_corner(hit: &Hit, new_size: f64, precise_box: Option<&Aabb>) -> Q3 {
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

/// The `size` voxel `hit` struck, for the eraser's terrain dig: as
/// world_adjacent_corner but staying inside the struck box on the normal axis.
#[allow(clippy::needless_range_loop)]
pub fn world_struck_corner(hit: &Hit, size: f64) -> Q3 {
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

/// The placed object struck: a cube, or a decor someone placed.
pub fn resolve_hit_object(hit: &Hit) -> Option<Entry> {
    match hit.struck {
        Struck::Object(at)
        | Struck::Decor(Instance {
            source: Source::Object(at),
            ..
        }) => World::now().placed(at),
        Struck::Ground | Struck::Objects | Struck::Decor(_) => None,
    }
}

/// In the root frame, from the eye.
/// Decor is struck where the frame drawing at `drawn` holds it. The world
/// asked is the stored one (`World::now`), not as the eraser's preview draws
/// it: what to erase is what is stored.
pub fn ray_pick(origin: V3, direction: V3, drawn: Tau) -> Option<Hit> {
    let ray = Ray {
        origin,
        direction,
        reach: PICK_REACH_U,
    };
    // A coarser tile's answer is a summary, not what is held.
    World::now()
        .striking_decor(drawn)
        .first_hit(&ray, held())
        .exact()
        .flatten()
}

#[cfg(test)]
#[path = "pick.test.rs"]
mod test;
