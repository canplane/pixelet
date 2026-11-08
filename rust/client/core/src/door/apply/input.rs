// pxl-core/door/apply/input.rs

//! The INPUT major's commands: the pointer, the wheel, the held movement
//! keys, and a tracker's pose.

use super::State;
use super::{ask_turn, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::edit::{clear_hover, clear_hover_coord};
use crate::domain::control::input::{
    camera_keys, camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up,
    camera_wheel, pinching,
};
use crate::door::cmd as abi;
use crate::pointer::tap;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn pointer(c: abi::InputPointer) -> Outcome {
    match c.phase {
        abi::PointerPhase::Down => {
            camera_pointer_down(c.pointer_type, c.id, c.button, c.x, c.y);
        }
        abi::PointerPhase::Move => camera_pointer_move(c.pointer_type, c.id, c.x, c.y),
        abi::PointerPhase::Up => {
            if let Some([x, y]) = camera_pointer_up(c.pointer_type, c.id) {
                tap(x, y);
            }
        }
        abi::PointerPhase::Out => {
            camera_pointer_out();
            clear_hover();
        }
        abi::PointerPhase::Cancel => {
            camera_pointer_up(c.pointer_type, c.id);
        }
    }
    // A pinch does not point at anything the tool could place.
    if pinching() {
        clear_hover_coord();
    }
    Ok(Some(0))
}

pub(super) fn wheel(c: abi::InputWheel) -> Outcome {
    camera_wheel(c.delta_y);
    Ok(Some(0))
}

pub(super) fn r#move(c: abi::InputMove) -> Outcome {
    camera_keys(c.x, c.y);
    Ok(Some(0))
}

pub(super) fn xr_pose(s: &mut State, c: abi::InputXrPose) -> Outcome {
    // The protocol sends a unit quaternion. One of zero length describes no
    // rotation and is refused.
    refuse_unless(
        c.qx * c.qx + c.qy * c.qy + c.qz * c.qz + c.qw * c.qw > 0.0,
        Errno::EINVAL,
    )?;
    controller::tracked_pose([c.x, c.y, c.z], [c.qx, c.qy, c.qz, c.qw]);
    ask_turn(s)
}

pub(super) fn xr_pose_end(s: &mut State) -> Outcome {
    controller::tracked_end();
    ask_turn(s)
}
