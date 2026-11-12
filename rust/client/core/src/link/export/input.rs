// pxl-core/link/export/input.rs

//! The INPUT major's commands: the pointer, the wheel, a trackpad's pinch,
//! the held movement keys, and a tracker's pose.

use super::{applied, refuse_unless};
use crate::domain::control::camera::controller;
use crate::domain::control::edit::{clear_hover, clear_hover_coord};
use crate::domain::control::input::{
    camera_keys, camera_pinch, camera_pointer_down, camera_pointer_move, camera_pointer_out,
    camera_pointer_up, camera_wheel, pinching,
};
use crate::abi::input::{PointerPhase, PointerType};
use crate::pointer::tap;
use pxl_global::Global;
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

pub(super) fn typing(typing: bool) -> Result<(), Errno> {
    pointers().typing = typing;
    Ok(())
}

/// A pointer pressed while the UI types is spent: its lift is a cancel.
fn phase_of(phase: PointerPhase, id: i32) -> PointerPhase {
    let t = pointers();
    let at = t.spent.iter().position(|&spent| spent == id);
    match (phase, at) {
        (PointerPhase::Down, None) if t.typing => {
            t.spent.push(id);
            phase
        }
        (PointerPhase::Up | PointerPhase::Cancel, Some(at)) => {
            t.spent.swap_remove(at);
            PointerPhase::Cancel
        }
        _ => phase,
    }
}

pub(super) fn pointer(phase: PointerPhase, pointer_type: PointerType, id: i32, button: i32, x: f64, y: f64) -> Result<(), Errno> {
    match phase_of(phase, id) {
        PointerPhase::Down => {
            camera_pointer_down(pointer_type, id, button, x, y);
        }
        PointerPhase::Move => camera_pointer_move(pointer_type, id, x, y),
        PointerPhase::Up => {
            if let Some([x, y]) = camera_pointer_up(pointer_type, id) {
                tap(x, y);
            }
        }
        PointerPhase::Out => {
            camera_pointer_out();
            clear_hover();
        }
        PointerPhase::Cancel => {
            camera_pointer_up(pointer_type, id);
        }
    }
    // A pinch does not point at anything the tool could place.
    if pinching() {
        clear_hover_coord();
    }
    Ok(())
}

pub(super) fn wheel(delta_y: f64) -> Result<(), Errno> {
    camera_wheel(delta_y);
    Ok(())
}

pub(super) fn pinch(scale: f64) -> Result<(), Errno> {
    refuse_unless(scale > 0.0, Errno::EINVAL)?;
    camera_pinch(scale);
    Ok(())
}

pub(super) fn r#move(x: i32, y: i32) -> Result<(), Errno> {
    camera_keys(x, y);
    Ok(())
}

pub(super) fn xr_pose(x: f64, y: f64, z: f64, qx: f64, qy: f64, qz: f64, qw: f64) -> Result<(), Errno> {
    // The protocol sends a unit quaternion. One of zero length describes no
    // rotation and is refused.
    refuse_unless(
        qx * qx + qy * qy + qz * qz + qw * qw > 0.0,
        Errno::EINVAL,
    )?;
    controller::tracked_pose([x, y, z], [qx, qy, qz, qw]);
    applied()
}

pub(super) fn xr_pose_end() -> Result<(), Errno> {
    controller::tracked_end();
    applied()
}
