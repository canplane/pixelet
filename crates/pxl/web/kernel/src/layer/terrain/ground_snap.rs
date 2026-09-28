// pxl-kernel/layer/terrain/ground_snap.rs

//! The map center standing on the ground: where the camera pivots over a
//! coordinate, and the pivot snapped in again when the ground under it
//! changes.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;
use crate::world::access::{Access, FINEST};

/// Where the camera's pivot sits on the ground at a coordinate's X/Y: the
/// center of the cube standing on the ground drawn there — the cell the
/// character occupies, the cell a placement's ghost fills — its original Z
/// discarded. None where no page over it is resident.
///
/// A center, not the top face, because everything the camera pivots on is a
/// cube: a tap on an object pivots on that cube's center
/// (object/geometry.rs's object_center), and a tap on the ground on the
/// center of the cube above it (camera/input). A pivot that merely sits on
/// the ground has to agree with those, or the first pan after a tap moves the
/// camera by the half cell between the two. Standing things (the puck,
/// decor, a place) are grounded to the top face itself; this is the one
/// place the camera's own convention is applied.
pub fn ground_pivot(at: &Pvf3) -> Option<Pvf3> {
    let ground = Access::now().at(at, FINEST).value()?;
    let mut out = Pvf3::default();
    pvf3::gather(&mut out, pvf3::X(at), pvf3::Y(at), pvf3::from_real(ground.top + U_PER_CELL / 2.0));
    Some(out)
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

/// Re-samples the ground under the current map_center and snaps it in, if
/// ground_snap is on. Shared by a page's arrival (layer/terrain/page.rs) and
/// by any one-shot interaction (e.g. the compass's reset-orientation click)
/// that should also leave the view properly grounded instead of only fixing
/// angles/distance.
pub fn reground_map_center() {
    if !settings_store().ground_snap {
        return;
    }
    let Some(map_center) = map_store().map_center else { return };
    let Some(grounded) = ground_pivot(&map_center) else { return };
    if grounded != map_center {
        map_store().map_center = Some(grounded);
    }
}
