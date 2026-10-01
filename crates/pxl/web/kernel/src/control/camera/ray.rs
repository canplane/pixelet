// pxl-kernel/control/camera/ray.rs

//! The pointer's pick: the one ray the whole world is hit by — terrain (any
//! face: top or a cliff/wall side) and placed objects (any size, including
//! sub-cell) alike — asked of the access (world/access.rs `first_hit`) at
//! terrain's finest level, which is the level the ground about the eye is
//! drawn at, so what the pick meets is what is on the screen there.
//!
//! What this module adds is how far the pick reaches, and what a hit means
//! for a placement: the corner a new cube takes, the voxel the eraser digs, and
//! which placed object a hit is. The ray itself is the view's
//! (world/frame/view.rs), in the root frame the chunks are placed in.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::V3;

use crate::layer::object::geometry::Aabb;
use crate::layer::object::store::{get_placed_object, OccupiedEntry};
use crate::world::access::{Access, Detail, Hit, Ray, Struck, FINEST};
use crate::world::frame::coord::{local_world_origin, local_world_to_canonical};

/// How far from the eye the pick reaches, in units: what can be edited is a
/// sphere about the observer, whichever controller placed it, and the ray's
/// own length is that sphere — every hit along it lies within it. A product
/// number: 32 cells.
const PICK_REACH_U: f64 = 32.0 * U_PER_CELL;

/// The struck column's top terrain cell's own (min-x, min-y, min-z) CORNER —
/// for the pointer tool's hover cursor over plain terrain OR a wall (both give
/// the same "which cell" answer via box_min's own XY, since a wall hit's box is
/// still that wall's OWN cell, not a neighbor's). X/Y come straight from
/// box_min; Z is the drawn surface there stepped DOWN one cell, so the anchor
/// is the top voxel's floor, not its top face — the same min-corner
/// convention world_struck_corner / world_adjacent_corner already use, which
/// lets the ghost and the recenter both apply a symmetric + half a cell on all
/// three axes. Objects don't go through this — see resolve_hit_object.
pub fn world_hover_pvf3(hit: &Hit, map_origin: IVec) -> Option<Pvf3> {
    // The cell's middle, so the cell it names does not rest on a rounding
    // of its corner.
    let mut at = Pvf3::default();
    local_world_to_canonical(&mut at, &[hit.box_min[0] + U_PER_CELL / 2.0, hit.box_min[1] + U_PER_CELL / 2.0, 0.0], map_origin);
    let top = Access::now().at(&at, FINEST).value()?.top;
    let mut corner = Pvf3::default();
    local_world_to_canonical(&mut corner, &[hit.box_min[0], hit.box_min[1], 0.0], map_origin);
    let (x, y) = (pvf3::X(&corner), pvf3::Y(&corner));
    pvf3::gather(&mut corner, x, y, pvf3::from_real(top - U_PER_CELL));
    Some(corner)
}

// Tolerance for "same nominal size"/"integer-aligned" comparisons below —
// bigger than the sub_cell 1/256-grid round trip's own worst-case error
// (±U_PER_CELL/512, volume.rs's pack/unpack pair), smaller than the gap
// between any two OBJECT_SIZES tiers (smallest U_PER_CELL/8).
const SAME_EXTENT_EPS: f64 = U_PER_CELL / 128.0;

/// Smallest multiple of `size` that is >= v (epsilon-fuzzed so a v already ON
/// the grid stays put instead of bumping to the next line on float noise) —
/// the sparse_tree octree only ever stores a `size`-tier object at one of these
/// lines, on EVERY axis, so this is the one honest "where can a new_size box's
/// near edge legally start" answer; anything else just gets silently
/// re-snapped by the write itself later, which is exactly what caused the
/// overlap bug this fixes.
fn grid_ceil(v: f64, size: f64) -> f64 {
    ((v - SAME_EXTENT_EPS) / size).ceil() * size
}

/// Largest multiple of `size` that is <= v — grid_ceil's mirror.
fn grid_floor(v: f64, size: f64) -> f64 {
    ((v + SAME_EXTENT_EPS) / size).floor() * size
}

/// The corner (min-x/y/z) a new `new_size` cube would occupy flush against
/// whatever `hit` struck — ONE rule, applied identically to all three axes:
/// every axis lands on new_size's own sparse_tree-canonical grid, and which of
/// the two grid-snap directions applies is purely a function of that axis's
/// own normal component:
/// - n > 0 (this axis IS the struck face's outward normal, positive side): the
///   new box must start at-or-beyond the struck face — grid_ceil(box_max).
/// - n < 0 (outward normal, negative side): the new box must end at-or-before
///   the struck face — grid_floor(box_min) - new_size.
/// - n == 0 (this axis is untouched by the struck face): whichever
///   new_size-grid cell the hit's point falls into — grid_floor(point).
///   Automatically stays within the struck box's own footprint when new_size
///   <= the struck box's own extent on this axis, and automatically resolves
///   to the struck box's own containing cell when new_size is bigger.
///
/// A struck face can put its nonzero n on ANY axis (a side wall's normal is X
/// or Y, not just a top hit's Z), so one rule, chosen per axis by that axis's
/// own normal sign, is both simpler and the only version that can't
/// disagree with itself.
///
/// `precise_box`: the box of what the caller resolved the hit to instead of
/// `hit.box_min/box_max` — a decor met in front of what the ray struck.
///
/// Each axis is a multiple of `new_size`, which float holds exactly, and the
/// map origin is whole units, so the corner lies on the cube's own lattice:
/// the hover preview, the overlap check and the write, which all read this
/// PVF3, and the store, which files a cube on its lattice (store.rs
/// `At::of_cube`), agree on the one cube.
#[allow(clippy::needless_range_loop)]
pub fn world_adjacent_corner(hit: &Hit, new_size: f64, map_origin: IVec, precise_box: Option<&Aabb>) -> Pvf3 {
    let box_min = precise_box.map_or(hit.box_min, |b| b.min);
    let box_max = precise_box.map_or(hit.box_max, |b| b.max);
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

    let mut p = Pvf3::default();
    local_world_to_canonical(&mut p, &out, map_origin);
    p
}

/// The single real `size`-tall voxel `hit` actually struck — eraser's own
/// targeting for its "nothing placed here, dig into the terrain itself" case.
///
/// Same per-axis shape as world_adjacent_corner but the OPPOSITE choice on the
/// axis with a nonzero normal: world_adjacent_corner steps OUTSIDE the struck
/// box on that axis; this stays INSIDE it, landing on the exact `size`-tall
/// slice the struck face itself bounds. The axes with no normal component
/// use the hit's point, which falls inside the one cell that was struck.
#[allow(clippy::needless_range_loop)]
pub fn world_struck_corner(hit: &Hit, size: f64, map_origin: IVec) -> Pvf3 {
    let mut out: V3 = [0.0; 3];
    for axis in 0..3 {
        let n = hit.normal[axis];
        out[axis] = if n > 0.0 {
            grid_ceil(hit.box_max[axis], size) - size
        } else if n < 0.0 {
            grid_floor(hit.box_min[axis], size)
        } else {
            grid_floor(hit.point[axis], size)
        };
    }
    let mut p = Pvf3::default();
    local_world_to_canonical(&mut p, &out, map_origin);
    p
}

/// Which placed object `hit` struck: the one the access met, by where the
/// store files it. Decor is never resolved here — it paints no voxel and has
/// its own hit test (layer/decor/hit.rs).
pub fn resolve_hit_object(hit: &Hit) -> Option<OccupiedEntry> {
    match hit.struck {
        Struck::Object(at) => get_placed_object(at),
        Struck::Ground | Struck::Objects => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickResult {
    /// Never absent — ray_pick itself returns None when nothing was struck.
    pub hit: Hit,
}

/// The pick — see this module's own doc comment. The ray is in the root
/// frame, from the eye, and so is the hit. What the query lacked is not asked
/// for here: the pick is near the eye, where the view's own faults already
/// ask for the ground it is drawn from.
pub fn ray_pick(origin: V3, direction: V3) -> Option<PickResult> {
    let map_origin = local_world_origin();
    let frame: V3 = [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin)), ivec::to_real(ivec::Z(map_origin))];
    let ray = Ray {
        origin: std::array::from_fn(|k| origin[k] + frame[k]),
        direction,
        reach: PICK_REACH_U,
    };
    let hit = Access::now().first_hit(&ray, Detail::Level(FINEST)).value().flatten()?.moved(frame.map(|v| -v));
    Some(PickResult { hit })
}

#[cfg(test)]
#[path = "ray.test.rs"]
mod test;
