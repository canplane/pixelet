// pxl-kernel/layer/object/geometry.rs

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::Aabb;

use crate::layer::object::store::OccupiedEntry;

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Pvf3) -> Pvf3 {
    let half = pvf3::from_real(U_PER_CELL / 2.0);
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half);
    out
}

/// Relative to `map_origin`, the space the pick works in.
pub fn object_aabb(obj: &OccupiedEntry, map_origin: IVec) -> Aabb {
    let min = crate::world::frame::coord::canonical_to_local(&Pvf3 { i: obj.i, f: obj.f }, map_origin);
    let max = [min[0] + obj.size, min[1] + obj.size, min[2] + obj.size];
    Aabb { min, max }
}
