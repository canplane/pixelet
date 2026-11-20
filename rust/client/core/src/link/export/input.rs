// pxl-core/link/export/input.rs

//! The input interface's methods: what the hand does to the view as the UI
//! recognizes it, and the held movement keys.

use super::refuse_unless;
use crate::domain::control::edit::{clear_hover, clear_hover_coord};
use crate::domain::control::input::{
    camera_drag, camera_hover, camera_keys, camera_out, camera_pan, camera_rotate, camera_tilt,
    camera_zoom, hovering, tap_ndc,
};
use crate::pointer;
use xpute_kit::status::errno::Errno;

pub(super) fn pan(dx: f64, dy: f64) -> Result<(), Errno> {
    camera_pan(dx, dy);
    Ok(())
}

pub(super) fn rotate(rad: f64) -> Result<(), Errno> {
    camera_rotate(rad);
    Ok(())
}

pub(super) fn tilt(rad: f64) -> Result<(), Errno> {
    camera_tilt(rad);
    Ok(())
}

pub(super) fn zoom(scale: f64) -> Result<(), Errno> {
    refuse_unless(scale > 0.0, Errno::EINVAL)?;
    camera_zoom(scale);
    Ok(())
}

/// A hand dragging the view points at nothing the tool could place.
pub(super) fn drag(dragging: bool) -> Result<(), Errno> {
    camera_drag(dragging);
    if dragging {
        clear_hover_coord();
    }
    Ok(())
}

pub(super) fn tap(x: f64, y: f64) -> Result<(), Errno> {
    let [x, y] = tap_ndc(x, y);
    pointer::tap(x, y);
    // A tap leaves its hover only under a pointer that stays over the world,
    // as a finger does not.
    if !hovering() {
        clear_hover();
    }
    Ok(())
}

pub(super) fn hover(x: f64, y: f64) -> Result<(), Errno> {
    camera_hover(x, y);
    Ok(())
}

pub(super) fn out() -> Result<(), Errno> {
    camera_out();
    clear_hover();
    Ok(())
}

pub(super) fn r#move(x: i32, y: i32) -> Result<(), Errno> {
    camera_keys(x, y);
    Ok(())
}
