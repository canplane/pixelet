// pxl-kernel/layer/object/geometry.rs

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::v3::Aabb;

use crate::layer::object::store::Entry;

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Pvf3) -> Pvf3 {
    let half = pvf3::from_real(U_PER_CELL / 2.0);
    pvf3::gather(pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half)
}

/// In the world's doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = crate::world::frame::coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}
