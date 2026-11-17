// pxl-core/domain/control/input.rs

//! The pointer's gestures and the movement keys, and what each asks of the
//! camera: pan, rotate, pinch with its twist and tilt, the wheel, and a
//! trackpad's pinch, which the platform hands over as a scale. A pan
//! by hand ends a pending destination, and a pointer down ends following
//! the device. What the pointer is over, its hover and its tap, belongs to
//! pxl-core pointer.rs. Events only gather what they ask, and the camera is
//! written once a frame (`camera_input_step`). When what the pointer holds
//! changes under a still pointer, its hover is picked again
//! (`camera_rehover`).

use crate::spec::step::InvalidationReason;
use std::collections::BTreeMap;

use crate::domain::control::param::{
    AXIS_LOCK_THRESHOLD_PX, KEY_PAN_PER_S, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX,
    TAP_DRAG_THRESHOLD_PX, TILT_SWEEP_FRACTION,
};

use crate::abi::input::Drag;
use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::{Active, active};
use crate::domain::control::camera::gesture::{Anchor, linear};
use crate::domain::control::camera::orbit;
use crate::domain::control::camera::state::{IntentPatch, camera_state};
use crate::domain::control::navigation::destination::cancel as cancel_destination;
use crate::domain::control::navigation::follow_mode::set_following;
use crate::domain::setting::settings;
use crate::domain::view::frame::screen::size;
use crate::sys::sched::invalidate;
use pxl_global::Global;
use xpute_guest::clock::now;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    None,
    Pan,
    Rotate,
    Pinch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RotateAxisLock {
    None,
    Tilt,
    Rotate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PointerPos {
    x: f64,
    y: f64,
}

struct Input {
    drag_mode: DragMode,
    did_drag: bool,
    pointer_x: f64,
    pointer_y: f64,
    last_x: f64,
    last_y: f64,
    // A tap is told from a drag by the total displacement since the pointer
    // went down, not by the moves between events.
    tap_start_x: f64,
    tap_start_y: f64,
    /// A mouse is over the canvas and not dragging. Its hover is picked
    /// again when what the pointer holds changes (`camera_rehover`).
    hovering: bool,
    hover_pending: bool,

    drag_pending: bool,
    /// The drag that ended with a move `camera_input_step` has not written, so
    /// that a release before the next frame loses none of it.
    released: DragMode,
    pending_pan_dx: f64,
    pending_pan_dy: f64,
    pending_view_x: f64,
    pending_view_y: f64,
    // The wheel's deltas are summed before a single `exp()`. That gives the
    // same zoom as one `exp()` per delta, since the exponents add.
    pending_wheel_delta_y: Option<f64>,
    /// A trackpad pinch's scales since the last step, multiplied.
    pending_pinch_scale: Option<f64>,

    /// The screen direction of the held keys, of unit length, or zero when
    /// none is held.
    key_dir: [f64; 2],
    /// The time the keys last panned, in ms.
    keys_at: f64,

    rotate_turn: Option<Anchor>,
    /// The tilt follows the drag's vertical travel since this anchor.
    rotate_tilt: Anchor,
    rotate_drag_start_x: f64,
    rotate_drag_start_y: f64,
    rotate_axis_lock: RotateAxisLock,

    active_pointers: BTreeMap<i32, PointerPos>,
    pinch_start_dist: f64,
    pinch_tilt: Anchor,
    /// Set once the fingers are far enough apart for their angle to be read
    /// (`MIN_TWIST_DIST_PX`).
    pinch_twist: Option<Anchor>,
}

static INPUT: Global<Input> = Global::new();

/// The input as a UI starts it: no pointer pressed, no key held, nothing
/// pending.
fn fresh() -> Input {
    Input {
        drag_mode: DragMode::None,
        did_drag: false,
        pointer_x: 0.0,
        pointer_y: 0.0,
        last_x: 0.0,
        last_y: 0.0,
        tap_start_x: 0.0,
        tap_start_y: 0.0,
        hovering: false,
        hover_pending: false,
        drag_pending: false,
        released: DragMode::None,
        pending_pan_dx: 0.0,
        pending_pan_dy: 0.0,
        pending_view_x: 0.0,
        pending_view_y: 0.0,
        pending_wheel_delta_y: None,
        pending_pinch_scale: None,
        key_dir: [0.0; 2],
        keys_at: 0.0,
        rotate_turn: None,
        rotate_tilt: Anchor::new(0.0, 0.0),
        rotate_drag_start_x: 0.0,
        rotate_drag_start_y: 0.0,
        rotate_axis_lock: RotateAxisLock::None,
        active_pointers: BTreeMap::new(),
        pinch_start_dist: 0.0,
        pinch_tilt: Anchor::new(0.0, 0.0),
        pinch_twist: None,
    }
}

fn input() -> &'static mut Input {
    INPUT.get(fresh)
}

/// Forgets what the UI that left had pressed and held, as a UI attaches.
pub(crate) fn forget() {
    INPUT.set(fresh());
}

/// Negative because dragging up tilts toward the horizon.
fn tilt_gain() -> f64 {
    -core::f64::consts::PI / (TILT_SWEEP_FRACTION * size()[1])
}

/// The polar angle a tilt asks for with the hand at `y`, a rotate drag's and a
/// pinch's alike, or none while the tilt is locked (`tilt_lock`).
fn tilt(anchor: &Anchor, y: f64) -> Option<f64> {
    (!settings().tilt_lock).then(|| anchor.ask(y, linear(tilt_gain())))
}

fn update_pointer(s: &mut Input, view_x: f64, view_y: f64) {
    let [width, height] = size();
    s.pointer_x = (view_x / width) * 2.0 - 1.0;
    s.pointer_y = -(view_y / height) * 2.0 + 1.0;
}

/// Runs a pan the hand makes. A pan by hand cancels a pending destination.
/// While the camera is tracked, nothing pans.
fn by_hand(pan: impl FnOnce()) {
    if active() != Active::Tracked {
        cancel_destination();
        pan();
    }
}

fn do_pan(dx: f64, dy: f64) {
    by_hand(|| controller::pan(dx, dy));
}

/// The azimuth follows the pointer's angle about the screen's center, as an
/// arcball does. The tilt moves at a fixed rate with the drag's travel from
/// its start. The drag locks to whichever axis dominates once it passes
/// `AXIS_LOCK_THRESHOLD_PX`.
fn do_rotate(s: &mut Input, view_x: f64, view_y: f64) {
    if s.rotate_axis_lock == RotateAxisLock::None {
        let total_dx = view_x - s.rotate_drag_start_x;
        let total_dy = view_y - s.rotate_drag_start_y;
        if libm::hypot(total_dx, total_dy) < AXIS_LOCK_THRESHOLD_PX {
            return;
        }
        s.rotate_axis_lock = if total_dy.abs() > total_dx.abs() {
            RotateAxisLock::Tilt
        } else {
            RotateAxisLock::Rotate
        };
    }

    if s.rotate_axis_lock == RotateAxisLock::Tilt {
        if let Some(polar) = tilt(&s.rotate_tilt, view_y) {
            let took = controller::set_intent(IntentPatch {
                polar: Some(polar),
                ..Default::default()
            });
            s.rotate_tilt.settle(polar, took.polar, view_y);
        }
        return;
    }

    let [width, height] = size();
    let dx = view_x - width / 2.0;
    let dyc = view_y - height / 2.0;
    // The angle is unstable near the screen's center, so the anchor is
    // dropped there and set again once the pointer moves away.
    if libm::hypot(dx, dyc) < MIN_ROTATE_RADIUS_PX {
        s.rotate_turn = None;
        return;
    }
    let angle = libm::atan2(dyc, dx);
    let turn = s
        .rotate_turn
        .get_or_insert(Anchor::new(camera_state().intent.azimuth, angle));
    let azimuth = turn.ask(angle, linear(-1.0));
    let took = controller::set_intent(IntentPatch {
        azimuth: Some(azimuth),
        ..Default::default()
    });
    turn.settle(azimuth, took.azimuth, angle);
}

fn pointer_pair(s: &Input) -> Option<(PointerPos, PointerPos)> {
    let mut it = s.active_pointers.values();
    Some((*it.next()?, *it.next()?))
}

/// Runs when the second finger goes down, and again when a third finger
/// lifts, so the gesture never jumps.
fn begin_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else {
        return;
    };
    s.pinch_start_dist = libm::hypot(p1.x - p0.x, p1.y - p0.y);
    controller::pinch_begin(s.pinch_start_dist);
    let camera = camera_state();
    s.pinch_tilt = Anchor::new(camera.intent.polar, (p0.y + p1.y) / 2.0);
    s.pinch_twist = None;
    s.drag_mode = DragMode::Pinch;
    s.did_drag = true;
}

fn update_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else {
        return;
    };
    if s.pinch_start_dist < 1.0 {
        return;
    }
    let dist = libm::hypot(p1.x - p0.x, p1.y - p0.y);
    let midpoint_y = (p0.y + p1.y) / 2.0;
    controller::pinch(dist);

    let intent = camera_state().intent;
    // A control that did not move is not asked, so a pinch that only zooms
    // writes no tilt.
    let polar = tilt(&s.pinch_tilt, midpoint_y).filter(|&p| p != intent.polar);
    let angle = libm::atan2(p1.y - p0.y, p1.x - p0.x);
    if dist < MIN_TWIST_DIST_PX {
        s.pinch_twist = None;
    }
    let twist = (dist >= MIN_TWIST_DIST_PX).then(|| {
        *s.pinch_twist
            .get_or_insert(Anchor::new(intent.azimuth, angle))
    });
    let azimuth = twist
        .map(|t| t.ask(angle, linear(-1.0)))
        .filter(|&a| a != intent.azimuth);
    if polar.is_none() && azimuth.is_none() {
        return;
    }
    let took = controller::set_intent(IntentPatch {
        dist: None,
        azimuth,
        polar,
    });
    if let Some(polar) = polar {
        s.pinch_tilt.settle(polar, took.polar, midpoint_y);
    }
    if let (Some(azimuth), Some(t)) = (azimuth, &mut s.pinch_twist) {
        t.settle(azimuth, took.azimuth, angle);
    }
}

pub(crate) fn camera_pointer_down(
    touch: bool,
    pointer_id: i32,
    drag: Drag,
    view_x: f64,
    view_y: f64,
) {
    let s = input();
    let camera = camera_state();
    if touch {
        s.hovering = false;
        s.active_pointers.insert(
            pointer_id,
            PointerPos {
                x: view_x,
                y: view_y,
            },
        );
        set_following(false);
        match s.active_pointers.len() {
            1 => {
                s.drag_mode = DragMode::Pan;
                s.did_drag = false;
                s.last_x = view_x;
                s.last_y = view_y;
                s.tap_start_x = view_x;
                s.tap_start_y = view_y;
                update_pointer(s, view_x, view_y);
            }
            2 => begin_pinch(s),
            _ => {}
        }
        return;
    }

    s.drag_mode = match drag {
        Drag::Pan => DragMode::Pan,
        Drag::Rotate => DragMode::Rotate,
        Drag::None => return,
    };
    s.did_drag = false;
    s.last_x = view_x;
    s.last_y = view_y;
    s.tap_start_x = view_x;
    s.tap_start_y = view_y;
    if s.drag_mode == DragMode::Rotate {
        // `do_rotate` sets the turn's anchor, because the down position may
        // lie within `MIN_ROTATE_RADIUS_PX` of the center.
        s.rotate_turn = None;
        s.rotate_axis_lock = RotateAxisLock::None;
        s.rotate_drag_start_x = view_x;
        s.rotate_drag_start_y = view_y;
        s.rotate_tilt = Anchor::new(camera.intent.polar, view_y);
    }
    update_pointer(s, view_x, view_y);
    set_following(false);
}

pub(crate) fn camera_pointer_move(touch: bool, pointer_id: i32, view_x: f64, view_y: f64) {
    let s = input();
    if touch {
        if let Some(pt) = s.active_pointers.get_mut(&pointer_id) {
            *pt = PointerPos {
                x: view_x,
                y: view_y,
            };
        }
        if s.drag_mode == DragMode::Pinch && s.active_pointers.len() >= 2 {
            s.drag_pending = true;
            return;
        }
        if s.drag_mode == DragMode::Pan && s.active_pointers.len() == 1 {
            if libm::hypot(view_x - s.tap_start_x, view_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
                s.did_drag = true;
            }
            s.pending_pan_dx += view_x - s.last_x;
            s.pending_pan_dy += view_y - s.last_y;
            s.last_x = view_x;
            s.last_y = view_y;
            s.drag_pending = true;
        }
        return;
    }

    update_pointer(s, view_x, view_y);
    if s.drag_mode == DragMode::None {
        s.hovering = true;
        s.hover_pending = true;
        return;
    }

    if libm::hypot(view_x - s.tap_start_x, view_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
        s.did_drag = true;
    }
    if s.drag_mode == DragMode::Pan {
        s.pending_pan_dx += view_x - s.last_x;
        s.pending_pan_dy += view_y - s.last_y;
    }
    s.last_x = view_x;
    s.last_y = view_y;
    s.pending_view_x = view_x;
    s.pending_view_y = view_y;
    s.drag_pending = true;
}

/// The tap's position in normalized device coordinates, if the pointer was
/// panning and stayed still until it let go.
fn tap_if_still(s: &Input, mode: DragMode) -> Option<[f64; 2]> {
    (!s.did_drag && mode == DragMode::Pan).then_some([s.pointer_x, s.pointer_y])
}

/// Ends the drag, keeping what it moved since the last frame for that frame
/// to write.
fn end_drag(s: &mut Input) {
    if s.drag_pending && matches!(s.drag_mode, DragMode::Pan | DragMode::Rotate) {
        s.released = s.drag_mode;
    }
    s.drag_mode = DragMode::None;
}

/// A pointer went up or was canceled. Returns where it tapped, if it was a
/// tap.
pub(crate) fn camera_pointer_up(touch: bool, pointer_id: i32) -> Option<[f64; 2]> {
    let s = input();
    if touch {
        s.active_pointers.remove(&pointer_id);
        match s.active_pointers.len() {
            0 => {
                let mode = s.drag_mode;
                end_drag(s);
                if mode != DragMode::None {
                    return tap_if_still(s, mode);
                }
            }
            1 => {
                // The gesture is already under way, so this lift is not a tap.
                let remaining = *s
                    .active_pointers
                    .values()
                    .next()
                    .or_bug(Errno::ENOTRECOVERABLE);
                s.drag_mode = DragMode::Pan;
                s.last_x = remaining.x;
                s.last_y = remaining.y;
                s.did_drag = true;
            }
            _ => begin_pinch(s),
        }
        return None;
    }
    if s.drag_mode == DragMode::None {
        return None;
    }
    let mode = s.drag_mode;
    end_drag(s);
    tap_if_still(s, mode)
}

pub(crate) fn camera_pointer_out() {
    input().hovering = false;
}

/// Called when what the pointer holds changes. A hovering pointer picks
/// again where it stands.
pub(crate) fn camera_rehover() {
    let s = input();
    if s.hovering {
        s.hover_pending = true;
    }
}

/// A wheel turn of `delta_y` CSS pixels.
pub(crate) fn camera_wheel(delta_y: f64) {
    *input().pending_wheel_delta_y.get_or_insert(0.0) += delta_y;
}

/// A trackpad's pinch, its fingers' spread changed by `scale`.
pub(crate) fn camera_pinch(scale: f64) {
    *input().pending_pinch_scale.get_or_insert(1.0) *= scale;
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

/// Writes into the camera what the pointer and the keys asked since the
/// last step. It runs once a frame.
pub(crate) fn camera_input_step() {
    let s = input();
    if s.key_dir != [0.0; 2] {
        let at = now();
        let share = KEY_PAN_PER_S * (at - s.keys_at) / 1000.0;
        s.keys_at = at;
        // A key moves the view the way it points. The keys' y runs down the
        // screen and the pan's runs up it, so y changes sign.
        let way = [s.key_dir[0] * share, -s.key_dir[1] * share];
        by_hand(|| controller::pan_across(way[0], way[1]));
        invalidate(InvalidationReason::View);
    }
    if s.drag_pending {
        s.drag_pending = false;
        let mode = match s.drag_mode {
            DragMode::None => s.released,
            held => held,
        };
        s.released = DragMode::None;
        match mode {
            DragMode::Pan => do_pan(s.pending_pan_dx, s.pending_pan_dy),
            DragMode::Rotate => {
                let (x, y) = (s.pending_view_x, s.pending_view_y);
                do_rotate(s, x, y);
            }
            DragMode::Pinch => update_pinch(s),
            DragMode::None => {}
        }
        s.pending_pan_dx = 0.0;
        s.pending_pan_dy = 0.0;
    }
    if let Some(delta_y) = s.pending_wheel_delta_y.take() {
        controller::wheel(delta_y);
    }
    if let Some(scale) = s.pending_pinch_scale.take() {
        controller::pinch_by(scale);
    }
}

/// Where the pointer is to be picked again, after the tick has placed the
/// camera. It is picked again when it moved over the screen, or when it
/// hovers still and the view `moved` under it (a wheel's zoom, a glide, the
/// keys). None while a drag is under way. pxl-core pointer.rs decides what
/// the pointer is over.
pub(crate) fn rehover_at(moved: bool) -> Option<[f64; 2]> {
    let s = input();
    let asked = core::mem::take(&mut s.hover_pending) || moved && s.hovering;
    (asked && s.drag_mode == DragMode::None).then_some([s.pointer_x, s.pointer_y])
}

pub(crate) fn pinching() -> bool {
    input().drag_mode == DragMode::Pinch
}

/// Whether the orbit's distance is moving: it is being pinched, or it is
/// settling after a wheel or a pinch.
pub(crate) fn zooming() -> bool {
    active() == Active::Orbit && (pinching() || orbit::zoom_settling())
}

/// Whether a pointer holds the camera: a pan, a rotate or a pinch is under
/// way.
pub(crate) fn dragging() -> bool {
    input().drag_mode != DragMode::None
}

/// Whether movement keys are held, so the view is walked rather than
/// dragged.
pub(crate) fn steering() -> bool {
    input().key_dir != [0.0; 2]
}
