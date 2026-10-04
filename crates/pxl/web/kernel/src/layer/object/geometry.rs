// pxl-kernel/layer/object/geometry.rs

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::Aabb;

use crate::layer::object::store::OccupiedEntry;

/// The center of the cell a hover anchor (`i`/`f`) points at — the point that
/// anchor actually *represents* to the player: where the camera recenters when
/// you tap, which is exactly the spot the hover-cursor ghost box is drawn
/// centered on. "Tap the highlighted box, the camera goes to that box."
///
/// `+ half` on every axis, symmetric — a hover anchor is a cell's (min-x,
/// min-y, min-z) corner on all three (camera/ray.rs `world_struck_corner`,
/// `world_adjacent_corner`), so an anchor at (x,y,z) occupies
/// (x,y,z)..(x+1,y+1,z+1) and its middle is (x+.5, y+.5, z+.5) — v3's own
/// span3/center3 convention, Minecraft's block model.
///
/// User-placed decor is centered the same way on X/Y but
/// keeps Z on the slot (a decor model's origin sits at its base, not its
/// middle) — the X/Y half of this, not the whole thing.
pub fn anchor_center(anchor: &Pvf3) -> Pvf3 {
    let half = pvf3::from_real(U_PER_CELL / 2.0);
    pvf3::gather(pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half)
}

/// Relative to `map_origin`, the space the pick works in.
pub fn object_aabb(obj: &Entry, map_origin: IVec) -> Aabb {
    let min = crate::world::frame::coord::canonical_to_local(&obj.corner(), map_origin);
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
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
