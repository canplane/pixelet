// pxl-kernel/layer/object/geometry.rs

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::fvec::FVec;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::V3;


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
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half);
    out
}

/// The corner (min-x, min-y, min-z) at `i`/`f` — the same convention the
/// mesher and the hover cursor render from (both add +size/2 on every axis to
/// get the visual center). Returned in map_origin-relative real units, the
/// same low-magnitude space the billboards' local positions and camera/ray.rs's
/// pick already operate in.
pub fn object_local_corner(i: IVec, f: FVec, map_origin: IVec) -> V3 {
    crate::world::frame::coord::canonical_to_local(&Pvf3 { i, f }, map_origin)
}

pub use pxl_spec::space::v3::Aabb;

/// An object's exact box, in map_origin-relative units: what a cube stacked
/// on it is placed flush against (camera/ray.rs `world_adjacent_corner`).
pub fn object_aabb(obj: &OccupiedEntry, map_origin: IVec) -> Aabb {
    let min = object_local_corner(obj.i, obj.f, map_origin);
    let max = [min[0] + obj.size, min[1] + obj.size, min[2] + obj.size];
    Aabb { min, max }
}
