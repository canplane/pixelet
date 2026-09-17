// pxl_core/module/camera/ray.rs

//! Single raycast entry point for the whole world — terrain (any face: top or
//! a cliff/wall side) and placed objects (any size, including sub-cell) alike,
//! since both are painted into the exact same sectioned per-column voxel field
//! the chunks are meshed from. One DDA walks the composite's own voxel-derived
//! boxes and returns whichever box the ray actually struck first, regardless
//! of whether it came from terrain or an object.
//!
//! The walk is the kernel's (crates/pxl/core/src/render/layer/pipeline/chunk_raycast.rs). What this
//! module adds is the interact radius, and what a hit means for a placement:
//! the corner a new cube takes, the voxel the eraser digs, and which placed
//! object a hit resolves to. The ray itself is the view's
//! (module/world/frame/view.rs), in the root frame the chunks are placed in.

use pxl_shared::bake::constant::U_PER_CELL;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::projection;
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};
use pxl_shared::spec::space::v3::V3;
use xpute_core::kit::mat4::Vec3;

use crate::module::world::frame::coord::{get_local_world_origin, local_world_to_canonical};
use crate::module::world::object::geometry::{aabb_overlap, object_aabb, AABB};
use crate::module::world::object::store::{get_objects_near, OccupiedEntry};
use crate::render::layer::pipeline::chunk_raycast::{ray_hit_world, RayLike, WorldHit};
use crate::module::world::terrain::elevation::with_ele;
use crate::render::layer::pipeline::volume::MaterialClasses;

const INTERACT_RADIUS_CELL: f64 = 32.0 / U_PER_CELL - 0.707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

pub fn in_interact_radius(a: &PVF3, b: &PVF3) -> bool {
    let dx = pvf3::to_real(pvf3::X(a)) - pvf3::to_real(pvf3::X(b));
    let dy = pvf3::to_real(pvf3::Y(a)) - pvf3::to_real(pvf3::Y(b));
    dx * dx + dy * dy <= INTERACT_RADIUS2_U
}

/// The struck top terrain cell's own (min-x, min-y, min-z) CORNER — for the
/// pointer tool's hover cursor over plain terrain OR a wall (both give the same
/// "which cell" answer via box_min's own XY, since a wall hit's box is still
/// that wall's OWN cell, not a neighbor's). X/Y come straight from box_min; Z
/// is the real terrain surface (with_ele) stepped DOWN one cell, so the anchor
/// is the struck voxel's floor, not its top face — the same min-corner
/// convention world_struck_corner / world_adjacent_corner already use, which
/// lets the ghost and the recenter both apply a symmetric + half a cell on all
/// three axes. Objects don't go through this — see resolve_hit_object.
pub fn world_hover_pvf3(hit: &WorldHit, map_origin: IVec) -> Option<PVF3> {
    // -U_PER_CELL, not 0: with_ele adds the real surface height onto this Z, so
    // starting one cell low lands the result on the struck top voxel's floor
    // (min-z corner) instead of its top face.
    let mut p = PVF3::default();
    projection::project(&mut p, hit.box_min[0], hit.box_min[1], -U_PER_CELL);
    p.i = ivec::add(p.i, map_origin);
    let (i, f) = (p.i, p.f);
    with_ele(&mut p, &PVF3 { i, f })
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
/// own face_normal component:
/// - n > 0 (this axis IS the struck face's outward normal, positive side): the
///   new box must start at-or-beyond the struck face — grid_ceil(box_max).
/// - n < 0 (outward normal, negative side): the new box must end at-or-before
///   the struck face — grid_floor(box_min) - new_size.
/// - n == 0 (this axis is untouched by the struck face): whichever
///   new_size-grid cell hit_point itself falls into — grid_floor(hit_point).
///   Automatically stays within the struck box's own footprint when new_size
///   <= the struck box's own extent on this axis, and automatically resolves
///   to the struck box's own containing cell when new_size is bigger.
///
/// A struck face can put its nonzero n on ANY axis (a side wall's normal is X
/// or Y, not just a top hit's Z), so one rule, chosen per axis by that axis's
/// own face_normal sign, is both simpler and the only version that can't
/// disagree with itself.
///
/// `precise_box`: use the caller's exact, known AABB instead of
/// `hit.box_min/box_max` when available. hit's own box for a sub-cell object
/// comes back through volume.rs's u8-packed sub_cell payload — accurate to only
/// ~1/510 of a cell — so stacking off it compounded that error stack-to-stack,
/// visibly drifting. Terrain/unknown hits have no more precise source.
///
/// Also snaps a whole-cell(-or-larger) result to a real cell boundary — the
/// store can't store that tier at a fractional height, so this is the one
/// place to fix it: both the hover preview and the real placement read this
/// exact same PVF3, so snapping here is what keeps the preview, the
/// overlap/blocked check, and the actual write all agreeing on the same target
/// cell.
#[allow(clippy::needless_range_loop)]
pub fn world_adjacent_corner(hit: &WorldHit, new_size: f64, map_origin: IVec, precise_box: Option<&AABB>) -> PVF3 {
    let box_min = precise_box.map_or(hit.box_min, |b| b.min);
    let box_max = precise_box.map_or(hit.box_max, |b| b.max);
    let mut out: V3 = [0.0; 3];

    for axis in 0..3 {
        let n = hit.face_normal[axis];
        out[axis] = if n > 0.0 {
            grid_ceil(box_max[axis], new_size)
        } else if n < 0.0 {
            grid_floor(box_min[axis], new_size) - new_size
        } else {
            grid_floor(hit.hit_point[axis], new_size)
        };
    }

    // Whole-cell(-or-larger) snap to a real cell boundary — see this
    // function's own doc comment.
    if new_size >= U_PER_CELL - SAME_EXTENT_EPS {
        for v in out.iter_mut() {
            let cells = *v / U_PER_CELL;
            let frac = cells - cells.floor();
            if !(SAME_EXTENT_EPS..=1.0 - SAME_EXTENT_EPS).contains(&frac) {
                continue; // already (near-)integer.
            }
            *v = cells.ceil() * U_PER_CELL;
        }
    }

    let mut p = PVF3::default();
    local_world_to_canonical(&mut p, &out, map_origin);
    p
}

/// The single real `size`-tall voxel `hit` actually struck, exactly as
/// raycast against the live rendered voxel field — eraser's own targeting for
/// its "nothing placed here, dig into the terrain itself" case. Deliberately
/// NOT just hit.box_min/box_max directly — those bound whichever merged run the
/// scan combined same-material voxels into. A flat, uniformly-colored terrain
/// column routinely merges dozens of cells deep into ONE box, so box_min there
/// can sit far below the real surface, nowhere near what was actually visually
/// struck.
///
/// Same per-axis shape as world_adjacent_corner but the OPPOSITE choice on the
/// axis with a nonzero normal: world_adjacent_corner steps OUTSIDE the struck
/// box on that axis; this stays INSIDE it, landing on the exact `size`-tall
/// slice the struck face itself bounds. The axes with no face_normal component
/// use hit_point itself, since only hit_point is guaranteed to fall inside the
/// ONE real cell that was struck rather than the merged run's whole footprint.
#[allow(clippy::needless_range_loop)]
pub fn world_struck_corner(hit: &WorldHit, size: f64, map_origin: IVec) -> PVF3 {
    let mut out: V3 = [0.0; 3];
    for axis in 0..3 {
        let n = hit.face_normal[axis];
        out[axis] = if n > 0.0 {
            grid_ceil(hit.box_max[axis], size) - size
        } else if n < 0.0 {
            grid_floor(hit.box_min[axis], size)
        } else {
            grid_floor(hit.hit_point[axis], size)
        };
    }
    let mut p = PVF3::default();
    local_world_to_canonical(&mut p, &out, map_origin);
    p
}

/// Which solid placed object `hit` actually struck, when hit.material_class is
/// OBJECT — whichever placed object's own real AABB (full precision, including
/// sub-cell) overlaps the hit box. The voxel field carries no object identity,
/// so this is a small secondary lookup, not the raycast itself. Decor is never
/// resolved here — it never paints a voxel of its own and has its own hit test
/// (decor_hit.rs).
pub fn resolve_hit_object(hit: &WorldHit, map_origin: IVec) -> Option<OccupiedEntry> {
    if hit.material_class != MaterialClasses::OBJECT {
        return None;
    }
    let hit_aabb = AABB { min: hit.box_min, max: hit.box_max };

    // Only checks objects near the hit point (the containing cell plus its 26
    // neighbors), not every placed object in the world.
    let mut hit_point_abs = PVF3::default();
    local_world_to_canonical(&mut hit_point_abs, &hit.hit_point, map_origin);

    // Nearest-to-hit_point match, not first-in-order — two objects flush
    // against each other can both register as touching the same hit_aabb right
    // at their shared boundary, and picking whichever is closest to where the
    // ray landed resolves that correctly.
    let mut best = None;
    let mut best_dist2 = f64::INFINITY;
    for o in get_objects_near(&hit_point_abs, U_PER_CELL) {
        let aabb = object_aabb(&o, map_origin);
        if !aabb_overlap(&aabb, &hit_aabb) {
            continue;
        }
        let c = [(aabb.min[0] + aabb.max[0]) * 0.5, (aabb.min[1] + aabb.max[1]) * 0.5, (aabb.min[2] + aabb.max[2]) * 0.5];
        let (dx, dy, dz) = (c[0] - hit.hit_point[0], c[1] - hit.hit_point[1], c[2] - hit.hit_point[2]);
        let dist2 = dx * dx + dy * dy + dz * dz;
        if dist2 < best_dist2 {
            best = Some(o);
            best_dist2 = dist2;
        }
    }
    best
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickResult {
    /// Never absent — ray_pick itself returns None when nothing was struck.
    pub hit: WorldHit,
}

/// Single raycast entry point — see this module's own doc comment. The ray is
/// in the root frame.
pub fn ray_pick(origin: Vec3, direction: Vec3, map_center: Option<&PVF3>) -> Option<PickResult> {
    let map_center = map_center?;
    let map_origin = get_local_world_origin(Some(map_center));

    let hit = ray_hit_world(&RayLike { origin, direction }, map_origin, INTERACT_RADIUS_U * 1.5)?;

    let mut probe = PVF3::default();
    local_world_to_canonical(&mut probe, &hit.hit_point, map_origin);
    if !in_interact_radius(&probe, map_center) {
        return None;
    }
    Some(PickResult { hit })
}
