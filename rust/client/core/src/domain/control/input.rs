// pxl-core/domain/control/input.rs

//! What the hand asks of the camera, as the UI recognizes it: a pan, a
//! rotation, a tilt and a zoom, each a change since the last, whether a hand
//! holds the view, and the movement keys. Events gather what they ask, and
//! the camera is written once a frame (`camera_input_step`), where a gesture
//! is taken: a pan applied to the camera as it stands, and a rotation, a
//! tilt and a zoom to where it ends. A pan by hand takes the pivot
//! (navigation.rs), and a rotation by hand ends the view's turning with the
//! device's heading. What the pointer is over, its hover and its tap,
//! belongs to pxl-core pointer.rs; when what the pointer holds changes under
//! a still pointer, its hover is picked again (`camera_rehover`).

use crate::spec::step::InvalidationReason;

use crate::domain::control::camera::controller;
use crate::domain::control::navigation::{stop_tracking_heading, take_by_hand, takes_gesture};
use crate::domain::control::param::KEY_PAN_PER_S;
use crate::domain::setting::settings;
use crate::domain::view::frame::screen::px_to_ndc;
use crate::sys::sched::invalidate;
use pxl_global::Global;
use xpute_guest::clock::now;

struct Input {
    /// Whether a hand drags the view.
    dragging: bool,
    /// The pointer over the world, in normalized device coordinates. Its hover is picked again when what the pointer holds
    /// changes (`camera_rehover`).
    hover: Option<[f64; 2]>,
    hover_pending: bool,

    /// What the hand asked since the last step: the pan's pixels, the rotation's
    /// and the tilt's radians, and the zoom's spread, multiplied.
    pan: [f64; 2],
    rotate: f64,
    tilt: f64,
    zoom: f64,

    /// The screen direction of the held keys, of unit length, or zero when
    /// none is held.
    key_dir: [f64; 2],
    /// The time the keys last panned, in ms.
    keys_at: f64,
}

static INPUT: Global<Input> = Global::new();

/// The input as a UI starts it: no hand dragging, no key held, nothing asked.
fn fresh() -> Input {
    Input {
        dragging: false,
        hover: None,
        hover_pending: false,
        pan: [0.0; 2],
        rotate: 0.0,
        tilt: 0.0,
        zoom: 1.0,
        key_dir: [0.0; 2],
        keys_at: 0.0,
    }
}

fn input() -> &'static mut Input {
    INPUT.get(fresh)
}

/// Forgets what the UI that left had pressed and held, as a UI attaches.
pub(crate) fn forget() {
    INPUT.set(fresh());
}

/// Whether a gesture is on: a hand holds the view, or a key is held. A
/// wheel holds nothing: its zoom is taken and eased with no gesture.
pub(crate) fn gesturing() -> bool {
    let s = input();
    s.dragging || s.key_dir != [0.0; 2]
}

pub(crate) fn camera_pan(dx: f64, dy: f64) {
    let s = input();
    s.pan = [s.pan[0] + dx, s.pan[1] + dy];
}

pub(crate) fn camera_rotate(rad: f64) {
    input().rotate += rad;
}

pub(crate) fn camera_tilt(rad: f64) {
    input().tilt += rad;
}

pub(crate) fn camera_zoom(scale: f64) {
    input().zoom *= scale;
}

/// A hand takes or lets go of the view.
pub(crate) fn camera_drag(dragging: bool) {
    input().dragging = dragging;
}

pub(crate) fn camera_hover(x: f64, y: f64) {
    let s = input();
    s.hover = Some(px_to_ndc([x, y]));
    s.hover_pending = true;
}

/// Where a tap at (`x`, `y`) pixels is, in NDC.
pub(crate) fn tap_ndc(x: f64, y: f64) -> [f64; 2] {
    px_to_ndc([x, y])
}

/// Whether a pointer is over the world.
pub(crate) fn hovering() -> bool {
    input().hover.is_some()
}

pub(crate) fn camera_out() {
    input().hover = None;
}

/// Called when what the pointer holds changes. A hovering pointer picks
/// again where it stands.
pub(crate) fn camera_rehover() {
    let s = input();
    if s.hover.is_some() {
        s.hover_pending = true;
    }
}

/// The held movement keys. Each axis is -1, 0 or 1, with x to the right and
/// y down.
pub(crate) fn camera_keys(x: i32, y: i32) {
    let s = input();
    let len = libm::hypot(x as f64, y as f64);
    let key_dir = if len > 0.0 {
        [x as f64 / len, y as f64 / len]
    } else {
        [0.0; 2]
    };
    if s.key_dir == [0.0; 2] {
        s.keys_at = now();
    }
    s.key_dir = key_dir;
}

/// Writes into the camera what the hand and the keys asked since the last
/// step, where a gesture is taken (navigation.rs `takes_gesture`): on an
/// animation it is not, and what was asked is dropped. A pan takes the
/// pivot first (`take_by_hand`), and a rotation ends the view's turning with
/// the device's heading. It runs once a frame.
pub(crate) fn camera_input_step() {
    let s = input();
    let taken = takes_gesture();
    if s.key_dir != [0.0; 2] {
        let at = now();
        let share = KEY_PAN_PER_S * (at - s.keys_at) / 1000.0;
        s.keys_at = at;
        // A key moves the view the way it points. The keys' y runs down the
        // screen and the pan's runs up it, so y changes sign.
        if taken {
            take_by_hand();
            controller::pan_across(s.key_dir[0] * share, -s.key_dir[1] * share);
        }
        invalidate(InvalidationReason::View);
    }
    let [dx, dy] = core::mem::take(&mut s.pan);
    let rotate = core::mem::take(&mut s.rotate);
    // A tilt is not asked while the tilt is locked.
    let tilt = match settings().tilt_lock {
        true => 0.0,
        false => s.tilt,
    };
    s.tilt = 0.0;
    let zoom = core::mem::replace(&mut s.zoom, 1.0);
    if !taken {
        return;
    }
    if dx != 0.0 || dy != 0.0 {
        take_by_hand();
        controller::pan(dx, dy);
    }
    if rotate != 0.0 || tilt != 0.0 {
        controller::face_by(rotate, tilt);
    }
    if rotate != 0.0 {
        stop_tracking_heading();
    }
    if zoom != 1.0 {
        controller::zoom(zoom);
    }
}

/// Where the pointer is to be picked again, after the tick has placed the
/// camera. It is picked again when it moved over the screen, or when it
/// hovers still and the view `moved` under it (a wheel's zoom, an ease, the
/// keys). None while a hand drags the view. pxl-core pointer.rs decides what
/// the pointer is over.
pub(crate) fn rehover_at(moved: bool) -> Option<[f64; 2]> {
    let s = input();
    let asked = core::mem::take(&mut s.hover_pending) || moved;
    s.hover.filter(|_| asked && !s.dragging)
}

/// Whether a hand drags the view.
pub(crate) fn dragging() -> bool {
    input().dragging
}

/// Whether movement keys are held, so the view is walked rather than
/// dragged.
pub(crate) fn steering() -> bool {
    input().key_dir != [0.0; 2]
}
