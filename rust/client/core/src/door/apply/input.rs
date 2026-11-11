// pxl-core/door/apply/input.rs

//! The INPUT major's commands: the pointer, the wheel, a trackpad's pinch,
//! the held movement keys, and a tracker's pose.

use super::State;
use super::{ask_turn, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::edit::{clear_hover, clear_hover_coord};
use crate::domain::control::input::{
    camera_keys, camera_pinch, camera_pointer_down, camera_pointer_move, camera_pointer_out,
    camera_pointer_up, camera_wheel, pinching,
};
use crate::door::cmd as abi;
use crate::pointer::tap;
use xpute_core::global::Global;
use xpute_core::io::door::Outcome;
use xpute_kit::status::errno::Errno;

#[derive(Default)]
struct Pointers {
    /// The UI is taking typed text (INPUT_TYPING).
    typing: bool,
    /// The pointers pressed while it was, whose lifts tap nothing.
    spent: Vec<i32>,
}

static POINTERS: Global<Pointers> = Global::new();

fn pointers() -> &'static mut Pointers {
    POINTERS.get(Pointers::default)
}

pub(super) fn typing(c: abi::InputTyping) -> Outcome {
    pointers().typing = c.typing;
    Ok(Some(0))
}

/// Forgets what an earlier UI said and pressed.
pub(in crate::door) fn forget_pointers() {
    *pointers() = Pointers::default();
}

/// A pointer pressed while the UI types is spent: its lift is a cancel.
fn phase_of(c: &abi::InputPointer) -> abi::PointerPhase {
    let t = pointers();
    let at = t.spent.iter().position(|&id| id == c.id);
    match (c.phase, at) {
        (abi::PointerPhase::Down, None) if t.typing => {
            t.spent.push(c.id);
            c.phase
        }
        (abi::PointerPhase::Up | abi::PointerPhase::Cancel, Some(at)) => {
            t.spent.swap_remove(at);
            abi::PointerPhase::Cancel
        }
        _ => c.phase,
    }
}

pub(super) fn pointer(c: abi::InputPointer) -> Outcome {
    match phase_of(&c) {
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

pub(super) fn pinch(c: abi::InputPinch) -> Outcome {
    refuse_unless(c.scale > 0.0, Errno::EINVAL)?;
    camera_pinch(c.scale);
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
