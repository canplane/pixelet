// pxl-core/link/export/input.rs

//! The input interface's methods: the pointer, the wheel, a trackpad's pinch,
//! the held movement keys and a tracker's pose.

use super::{applied, refuse_unless};
use crate::abi::input::{Drag, PointerPhase};
use crate::domain::control::camera::controller;
use crate::domain::control::edit::{clear_hover, clear_hover_coord};
use crate::domain::control::input::{
    camera_keys, camera_pinch, camera_pointer_down, camera_pointer_move, camera_pointer_out,
    camera_pointer_up, camera_wheel, pinching,
};
use crate::pointer::tap;
use xpute_kit::status::errno::Errno;

pub(super) fn pointer(
    phase: PointerPhase,
    touch: bool,
    id: i32,
    drag: Drag,
    x: f64,
    y: f64,
) -> Result<(), Errno> {
    match phase {
        PointerPhase::Down => {
            camera_pointer_down(touch, id, drag, x, y);
        }
        PointerPhase::Move => camera_pointer_move(touch, id, x, y),
        PointerPhase::Up => {
            if let Some([x, y]) = camera_pointer_up(touch, id) {
                tap(x, y);
            }
        }
        PointerPhase::Out => {
            camera_pointer_out();
            clear_hover();
        }
        PointerPhase::Cancel => {
            camera_pointer_up(touch, id);
        }
    }
    // A pinch does not point at anything the tool could place.
    if pinching() {
        clear_hover_coord();
    }
    // A pointer moves the view or what it hovers, which a frame shows.
    applied()
}

pub(super) fn wheel(delta_y: f64) -> Result<(), Errno> {
    camera_wheel(delta_y);
    applied()
}

pub(super) fn pinch(scale: f64) -> Result<(), Errno> {
    refuse_unless(scale > 0.0, Errno::EINVAL)?;
    camera_pinch(scale);
    applied()
}

pub(super) fn r#move(x: i32, y: i32) -> Result<(), Errno> {
    camera_keys(x, y);
    applied()
}

pub(super) fn xr_pose(
    x: f64,
    y: f64,
    z: f64,
    qx: f64,
    qy: f64,
    qz: f64,
    qw: f64,
) -> Result<(), Errno> {
    // The protocol sends a unit quaternion. One of zero length describes no
    // rotation and is refused.
    refuse_unless(qx * qx + qy * qy + qz * qz + qw * qw > 0.0, Errno::EINVAL)?;
    controller::tracked_pose([x, y, z], [qx, qy, qz, qw]);
    Ok(())
}

pub(super) fn xr_pose_end() -> Result<(), Errno> {
    controller::tracked_end();
    Ok(())
}
