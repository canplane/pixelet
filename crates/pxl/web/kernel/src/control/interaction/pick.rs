// pxl-kernel/control/interaction/pick.rs

//! The pointer's pick, asked of the access at terrain's finest zoom, the leaves', which
//! is what is on the screen about the eye; and what a hit means for a
//! placement or an erase.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::LEAF_Z;
use pxl_spec::space::v3::V3;

use crate::layer::object::store::{get_placed_object, Entry};
use crate::world::access::{Access, Detail, Hit, Ray, Struck};
use crate::world::frame::coord::world_to_canonical;
use pxl_spec::space::v3::Aabb;

/// A product number: what can be edited is a sphere about the observer.
const PICK_REACH_U: f64 = 32.0 * U_PER_CELL;

// Above the 1/256-grid round trip's error (±U_PER_CELL/512), below the gap
// between any two OBJECT_SIZES tiers (U_PER_CELL/8).
const SAME_EXTENT_EPS: f64 = U_PER_CELL / 128.0;

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
pub fn world_adjacent_corner(hit: &Hit, new_size: f64, precise_box: Option<&Aabb>) -> Pvf3 {
    let Aabb { min: box_min, max: box_max } = precise_box.copied().unwrap_or(hit.cube);
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
pub fn world_struck_corner(hit: &Hit, size: f64) -> Pvf3 {
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

/// Decor is never resolved here; it paints no voxel and has its own hit test.
pub fn resolve_hit_object(hit: &Hit) -> Option<Entry> {
    match hit.struck {
        Struck::Object(at) => get_placed_object(at),
        Struck::Ground | Struck::Objects => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickResult {
    pub hit: Hit,
}

/// In the root frame, from the eye.
pub fn ray_pick(origin: V3, direction: V3) -> Option<PickResult> {
    let ray = Ray {
        origin,
        direction,
        reach: PICK_REACH_U,
    };
    // A coarser tile's answer is a summary, not what is held.
    let hit = Access::now().first_hit(&ray, Detail::Zoom(LEAF_Z)).exact().flatten()?;
    Some(PickResult { hit })
}

#[cfg(test)]
#[path = "pick.test.rs"]
mod test;
