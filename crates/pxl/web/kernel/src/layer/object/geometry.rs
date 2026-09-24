// pxl-kernel/layer/object/geometry.rs

use pxl_spec::space::pvf3::fvec::FVec;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::V3;

use crate::world::frame::coord::local_world_to_canonical;

use crate::layer::object::store::OccupiedEntry;

/// The center of the cell a hover anchor (`i`/`f`) points at — the point that
/// anchor actually *represents* to the player: where the camera recenters when
/// you tap, which is exactly the spot the hover-cursor ghost box is drawn
/// centered on. "Tap the highlighted box, the camera goes to that box."
///
/// `+ half` on every axis, symmetric — a hover anchor is a cell's (min-x,
/// min-y, min-z) corner on all three (camera/ray.rs: X/Y straight from
/// box_min, Z the surface stepped down one cell to the struck voxel's floor),
/// so an anchor at (x,y,z) occupies (x,y,z)..(x+1,y+1,z+1) and its middle is
/// (x+.5, y+.5, z+.5) — v3's own span3/center3 convention, Minecraft's block
/// model. object_center below is the placed-cube counterpart: the exact same
/// shape, since a cube's anchor is a true min corner too.
///
/// User-placed decor is centred the same way on X/Y but
/// keeps Z on the slot (a decor model's origin sits at its base, not its
/// middle) — the X/Y half of this, not the whole thing.
pub fn anchor_center(anchor: &Pvf3) -> Pvf3 {
    let half = pvf3::from_real(0.5);
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half);
    out
}

/// The corner (min-x, min-y, min-z) at `i`/`f` — the same convention the
/// mesher and the hover cursor render from (both add +size/2 on every axis to
/// get the visual center). Returned in map_origin-relative real units, the
/// same low-magnitude space the billboards' local positions and camera/ray.rs's
/// raycast already operate in.
pub fn object_local_corner(i: IVec, f: FVec, map_origin: IVec) -> V3 {
    let local = Pvf3 { i: ivec::sub(i, map_origin), f };
    [pvf3::to_real(pvf3::X(&local)), pvf3::to_real(pvf3::Y(&local)), pvf3::to_real(pvf3::Z(&local))]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: V3,
    pub max: V3,
}

pub fn object_aabb(obj: &OccupiedEntry, map_origin: IVec) -> Aabb {
    corner_aabb(obj.i, obj.f, obj.size, map_origin)
}

/// Same box shape object_aabb produces, for a candidate corner+size that isn't
/// (yet) a stored entry — e.g. a placement/overlap check before committing.
pub fn corner_aabb(i: IVec, f: FVec, size: f64, map_origin: IVec) -> Aabb {
    let min = object_local_corner(i, f, map_origin);
    let max = [min[0] + size, min[1] + size, min[2] + size];
    Aabb { min, max }
}

// Two AABBs must overlap by MORE than this to count — a flush-touching pair
// (one box's own face exactly against another's, e.g. stacking) is supposed
// to read as zero overlap, but both AABBs here are built by re-deriving real
// coordinates from PVF3 at each of possibly several chained placements — a
// strict `<` with no tolerance treats even a single ULP of drift either
// direction as a real verdict, silently flipping a legitimate flush placement
// to "blocked" (or the reverse). Comfortably bigger than realistic float64
// noise over a handful of chained conversions, comfortably smaller than the
// smallest real gap two placements could ever legitimately have (any
// OBJECT_SIZES tier, 1.0/8 at minimum).
const OVERLAP_EPS: f64 = 1e-6;

pub fn aabb_overlap(a: &Aabb, b: &Aabb) -> bool {
    a.min[0] < b.max[0] - OVERLAP_EPS
        && b.min[0] < a.max[0] - OVERLAP_EPS
        && a.min[1] < b.max[1] - OVERLAP_EPS
        && b.min[1] < a.max[1] - OVERLAP_EPS
        && a.min[2] < b.max[2] - OVERLAP_EPS
        && b.min[2] < a.max[2] - OVERLAP_EPS
}

/// This object's true center (corner + size/2 on every axis), as an absolute
/// PVF3 — e.g. for recentering the map on an object rather than its corner.
pub fn object_center(obj: &OccupiedEntry, map_origin: IVec) -> Pvf3 {
    let corner = object_local_corner(obj.i, obj.f, map_origin);
    let half = obj.size / 2.0;
    let center = [corner[0] + half, corner[1] + half, corner[2] + half];
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, &center, map_origin);
    out
}
