// pxl-kernel/layer/terrain/ground_snap.rs

//! The map center standing on the ground: where the camera pivots over a
//! coordinate.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::world::access::Access;

/// Where the camera's pivot sits from a coordinate: the center of the cube
/// standing on what supports a walker starting from the cell it lies in
/// (world/access.rs `support`) — the cell the character occupies. None
/// where no page over it is resident.
///
/// A center, not the top face, because the camera pivots on the cube the
/// character would fill. Every map center an interaction sets — a pan, a
/// tap — is this one, and the free camera's eye stands on the same support,
/// so the first pan after a tap moves nothing. Standing things (the puck, decor, a place) are grounded to the
/// top face itself; this is the one place the camera's own convention is
/// applied.
pub fn ground_pivot(at: &Pvf3) -> Option<Pvf3> {
    Some(pivot_over(at, Access::now().support(at).value()?))
}

/// The pivot over `at` on ground whose top is `top`.
pub fn pivot_over(at: &Pvf3, top: f64) -> Pvf3 {
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(top + U_PER_CELL / 2.0));
    out
}

/// ground_pivot for a coordinate, falling back to the coordinate itself when
/// no ground is resident under it yet.
///
/// The one "follow the actual ground" step every map_center-setting
/// interaction takes — a pan, the GPS recenter — so that all of them agree
/// on where the ground is. A recenter that set map_center to the device's raw
/// GPS altitude instead would put the camera at a different ground than a pan
/// does.
pub fn ground_or_raw(coord: &Pvf3) -> Pvf3 {
    ground_pivot(coord).unwrap_or(*coord)
}
