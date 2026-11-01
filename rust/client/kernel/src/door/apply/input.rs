// pxl-kernel/door/apply/input.rs

//! INPUT: the input's: the canvas's rect, the pointer, the wheel, the held movement, a tracker's pose.

use super::State;
use super::{ask_turn, given, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::camera::input::camera_input_rect;
use crate::domain::control::camera::input::camera_keys;
use crate::domain::control::camera::input::camera_pointer_down;
use crate::domain::control::camera::input::camera_pointer_move;
use crate::domain::control::camera::input::camera_pointer_out;
use crate::domain::control::camera::input::camera_pointer_up;
use crate::domain::control::camera::input::camera_wheel;
use crate::domain::control::cmd::{self as abi, Command};
use crate::pointer;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::INPUT_RECT => {
            let p = abi::InputRect::read(given(pkt)?)?;
            refuse_unless(p.width >= 0.0 && p.height >= 0.0, Errno::EINVAL)?;
            camera_input_rect(p);
            Ok(Some(0))
        }
        Command::INPUT_POINTER => {
            let p = abi::InputPointer::read(given(pkt)?)?;
            match p.phase {
                abi::PointerPhase::Down => {
                    camera_pointer_down(p.pointer_type, p.id, p.button, p.x, p.y);
                }
                abi::PointerPhase::Move => camera_pointer_move(p.pointer_type, p.id, p.x, p.y),
                abi::PointerPhase::Up => {
                    if let Some([x, y]) = camera_pointer_up(p.pointer_type, p.id) {
                        pointer::tap(x, y);
                    }
                }
                abi::PointerPhase::Out => camera_pointer_out(),
                abi::PointerPhase::Cancel => {
                    camera_pointer_up(p.pointer_type, p.id);
                }
            }
            Ok(Some(0))
        }
        Command::INPUT_WHEEL => {
            camera_wheel(abi::InputWheel::read(given(pkt)?)?.delta_y);
            Ok(Some(0))
        }
        Command::INPUT_MOVE => {
            let p = abi::InputMove::read(given(pkt)?)?;
            camera_keys(p.x, p.y);
            Ok(Some(0))
        }
        Command::INPUT_XR_POSE => {
            let p = abi::InputXrPose::read(given(pkt)?)?;
            // A unit quaternion by the protocol; one of no length turns nothing.
            refuse_unless(
                p.qx * p.qx + p.qy * p.qy + p.qz * p.qz + p.qw * p.qw > 0.0,
                Errno::EINVAL,
            )?;
            controller::tracked_pose([p.x, p.y, p.z], [p.qx, p.qy, p.qz, p.qw]);
            ask_turn(s)
        }
        Command::INPUT_XR_POSE_END => {
            controller::tracked_end();
            ask_turn(s)
        }
        _ => Ok(None),
    }
}
