// pxl-kernel/control/camera/input.rs

//! The pointer's gestures: pan, rotate, pinch/twist/tilt and wheel; hover
//! and tap go to interaction.rs. Store writes are coalesced to once a frame
//! (`camera_input_step`).
//! What the pointer holds comes with each event; when it changes under a
//! still pointer, the shell resends the move to re-resolve hover.

use std::collections::BTreeMap;

use camera_cfg::input::{AXIS_LOCK_THRESHOLD_PX, KEY_PAN_PX_PER_S, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX, TAP_DRAG_THRESHOLD_PX, TILT_SWEEP_FRACTION};

pub use crate::abi::cmd::PointerKind;
use crate::control::camera::config::camera_cfg;
use crate::control::camera::controller::{active, Active};
use crate::control::camera::gesture::{linear, Anchor};
use crate::control::camera::store::{camera_store, CameraStatus, IntentPatch};
use crate::control::interaction::{clear_hover, clear_hover_coord, hover_at, tap};
use crate::control::navigation::destination::cancel as cancel_destination;
use crate::shell::setting::store::settings_store;
use crate::world::change::request_world_tick;
use crate::world::view::frame::pose::view;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
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
    rect: Option<Rect>,
    last_x: f64,
    last_y: f64,
    // Total displacement from pointerdown, not per-event deltas, decides tap vs drag.
    tap_start_x: f64,
    tap_start_y: f64,
    /// A mouse over the canvas and not dragging, whose hover a new holding redoes.
    hovering: bool,
    hover_pending: bool,

    // Pointer moves fire far more often than frames; uncoalesced store writes
    // dominated a drag's CPU time in profiling.
    drag_pending: bool,
    pending_pan_dx: f64,
    pending_pan_dy: f64,
    pending_client_x: f64,
    pending_client_y: f64,
    // Summed before one exp(), which is exact.
    pending_wheel_delta_y: Option<f64>,

    // ---- movement keys ----
    /// The held keys' screen direction, unit length or zero.
    keys: [f64; 2],
    /// When the keys last panned, in ms.
    keys_at: f64,

    // ---- rotate-drag ----
    rotate_turn: Option<Anchor>,
    /// A tilt is the drag's vertical travel from this anchor.
    rotate_tilt: Anchor,
    rotate_drag_start_x: f64,
    rotate_drag_start_y: f64,
    rotate_axis_lock: RotateAxisLock,

    // ---- multi-touch ----
    active_pointers: BTreeMap<i32, PointerPos>,
    pinch_start_dist: f64,
    pinch_tilt: Anchor,
    /// Anchored once the fingers are far enough apart to read the angle.
    pinch_twist: Option<Anchor>,
}

static INPUT: Global<Input> = Global::new();

fn input() -> &'static mut Input {
    INPUT.get(|| Input {
        drag_mode: DragMode::None,
        did_drag: false,
        pointer_x: 0.0,
        pointer_y: 0.0,
        rect: None,
        last_x: 0.0,
        last_y: 0.0,
        tap_start_x: 0.0,
        tap_start_y: 0.0,
        hovering: false,
        hover_pending: false,
        drag_pending: false,
        pending_pan_dx: 0.0,
        pending_pan_dy: 0.0,
        pending_client_x: 0.0,
        pending_client_y: 0.0,
        pending_wheel_delta_y: None,
        keys: [0.0; 2],
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
    })
}

/// Negative because dragging up tilts toward the horizon.
fn tilt_gain() -> f64 {
    -core::f64::consts::PI / (TILT_SWEEP_FRACTION * view().height)
}

pub fn camera_input_rect(rect: Rect) {
    input().rect = Some(rect);
}

fn update_pointer(s: &mut Input, client_x: f64, client_y: f64) {
    let Some(rect) = s.rect else { return };
    s.pointer_x = ((client_x - rect.left) / rect.width) * 2.0 - 1.0;
    s.pointer_y = -((client_y - rect.top) / rect.height) * 2.0 + 1.0;
}

/// A pan puts the map center by hand, so a pending destination is canceled.
fn do_pan(dx: f64, dy: f64) {
    if active() != Active::Tracked {
        cancel_destination();
        crate::control::camera::controller::pan(dx, dy);
    }
}

/// Azimuth follows the cursor's angle about screen center (arcball), tilt a
/// fixed rate from the drag's start. The drag locks to whichever axis
/// dominates past AXIS_LOCK_THRESHOLD_PX: an off-center vertical drag still
/// sweeps an angle, which read as a spin once tilt saturated.
fn do_rotate(s: &mut Input, client_x: f64, client_y: f64) {
    if s.rotate_axis_lock == RotateAxisLock::None {
        let total_dx = client_x - s.rotate_drag_start_x;
        let total_dy = client_y - s.rotate_drag_start_y;
        if libm::hypot(total_dx, total_dy) < AXIS_LOCK_THRESHOLD_PX {
            return;
        }
        s.rotate_axis_lock = if total_dy.abs() > total_dx.abs() { RotateAxisLock::Tilt } else { RotateAxisLock::Rotate };
    }

    if s.rotate_axis_lock == RotateAxisLock::Tilt {
        // tilt_lock is an experiment toggle, not a decided product change.
        if !settings_store().tilt_lock {
            let polar = s.rotate_tilt.ask(client_y, linear(tilt_gain()));
            let took = crate::control::camera::controller::set_intent(IntentPatch {
                polar: Some(polar),
                ..Default::default()
            });
            s.rotate_tilt.settle(polar, took.polar, client_y);
        }
        return;
    }

    let Some(rect) = s.rect else { return };
    let dx = client_x - (rect.left + rect.width / 2.0);
    let dyc = client_y - (rect.top + rect.height / 2.0);
    // atan2 is unstable near the screen center: drop the anchor and re-anchor later.
    if libm::hypot(dx, dyc) < MIN_ROTATE_RADIUS_PX {
        s.rotate_turn = None;
        return;
    }
    let angle = libm::atan2(dyc, dx);
    let turn = s.rotate_turn.get_or_insert(Anchor::new(camera_store().intent.azimuth, angle));
    let azimuth = turn.ask(angle, linear(-1.0));
    let took = crate::control::camera::controller::set_intent(IntentPatch {
        azimuth: Some(azimuth),
        ..Default::default()
    });
    turn.settle(azimuth, took.azimuth, angle);
}

fn pointer_pair(s: &Input) -> Option<(PointerPos, PointerPos)> {
    let mut it = s.active_pointers.values();
    Some((*it.next()?, *it.next()?))
}

/// On the second finger down, and again when a third lifts, so the gesture never jumps.
fn begin_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    s.pinch_start_dist = libm::hypot(p1.x - p0.x, p1.y - p0.y);
    crate::control::camera::controller::pinch_begin(s.pinch_start_dist);
    let camera = camera_store();
    s.pinch_tilt = Anchor::new(camera.intent.polar, (p0.y + p1.y) / 2.0);
    s.pinch_twist = None;
    s.drag_mode = DragMode::Pinch;
    s.did_drag = true;
    clear_hover_coord();
}

fn update_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    if s.pinch_start_dist < 1.0 {
        return;
    }
    let dist = libm::hypot(p1.x - p0.x, p1.y - p0.y);
    let midpoint_y = (p0.y + p1.y) / 2.0;
    crate::control::camera::controller::pinch(dist);

    let intent = camera_store().intent;
    // An unmoved control is not asked, so a zoom-only pinch writes no tilt.
    let polar = s.pinch_tilt.ask(midpoint_y, linear(tilt_gain()));
    let polar = (!settings_store().tilt_lock && polar != intent.polar).then_some(polar);
    let angle = libm::atan2(p1.y - p0.y, p1.x - p0.x);
    if dist < MIN_TWIST_DIST_PX {
        s.pinch_twist = None;
    }
    let twist = (dist >= MIN_TWIST_DIST_PX).then(|| *s.pinch_twist.get_or_insert(Anchor::new(intent.azimuth, angle)));
    let azimuth = twist.map(|t| t.ask(angle, linear(-1.0))).filter(|&a| a != intent.azimuth);
    if polar.is_none() && azimuth.is_none() {
        return;
    }
    let took = crate::control::camera::controller::set_intent(IntentPatch { dist: None, azimuth, polar });
    if let Some(polar) = polar {
        s.pinch_tilt.settle(polar, took.polar, midpoint_y);
    }
    if let (Some(azimuth), Some(t)) = (azimuth, &mut s.pinch_twist) {
        t.settle(azimuth, took.azimuth, angle);
    }
}

pub fn camera_pointer_down(kind: PointerKind, pointer_id: i32, button: i32, client_x: f64, client_y: f64) {
    let s = input();
    let camera = camera_store();
    if kind == PointerKind::Touch {
        s.hovering = false;
        s.active_pointers.insert(pointer_id, PointerPos { x: client_x, y: client_y });
        camera.camera_status = CameraStatus::Drag;
        camera.is_follow_mode = false;
        match s.active_pointers.len() {
            1 => {
                s.drag_mode = DragMode::Pan;
                s.did_drag = false;
                s.last_x = client_x;
                s.last_y = client_y;
                s.tap_start_x = client_x;
                s.tap_start_y = client_y;
                update_pointer(s, client_x, client_y);
            }
            2 => begin_pinch(s),
            _ => {}
        }
        return;
    }

    s.drag_mode = match button {
        0 => DragMode::Pan,
        2 => DragMode::Rotate,
        _ => return,
    };
    s.did_drag = false;
    s.last_x = client_x;
    s.last_y = client_y;
    s.tap_start_x = client_x;
    s.tap_start_y = client_y;
    if s.drag_mode == DragMode::Rotate {
        // Anchored by do_rotate: the down position may be within MIN_ROTATE_RADIUS_PX.
        s.rotate_turn = None;
        s.rotate_axis_lock = RotateAxisLock::None;
        s.rotate_drag_start_x = client_x;
        s.rotate_drag_start_y = client_y;
        s.rotate_tilt = Anchor::new(camera.intent.polar, client_y);
    }
    update_pointer(s, client_x, client_y);
    camera.camera_status = CameraStatus::Drag;
    camera.is_follow_mode = false;
}

pub fn camera_pointer_move(kind: PointerKind, pointer_id: i32, client_x: f64, client_y: f64) {
    let s = input();
    if kind == PointerKind::Touch {
        if let Some(pt) = s.active_pointers.get_mut(&pointer_id) {
            *pt = PointerPos { x: client_x, y: client_y };
        }
        if s.drag_mode == DragMode::Pinch && s.active_pointers.len() >= 2 {
            s.drag_pending = true;
            return;
        }
        if s.drag_mode == DragMode::Pan && s.active_pointers.len() == 1 {
            if libm::hypot(client_x - s.tap_start_x, client_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
                s.did_drag = true;
            }
            s.pending_pan_dx += client_x - s.last_x;
            s.pending_pan_dy += client_y - s.last_y;
            s.last_x = client_x;
            s.last_y = client_y;
            s.drag_pending = true;
        }
        return;
    }

    update_pointer(s, client_x, client_y);
    if s.drag_mode == DragMode::None {
        s.hovering = true;
        s.hover_pending = true;
        return;
    }

    if libm::hypot(client_x - s.tap_start_x, client_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
        s.did_drag = true;
    }
    if s.drag_mode == DragMode::Pan {
        s.pending_pan_dx += client_x - s.last_x;
        s.pending_pan_dy += client_y - s.last_y;
    }
    s.last_x = client_x;
    s.last_y = client_y;
    s.pending_client_x = client_x;
    s.pending_client_y = client_y;
    s.drag_pending = true;
}

fn tap_if_still(s: &Input, mode: DragMode) {
    if !s.did_drag && mode == DragMode::Pan {
        tap(s.pointer_x, s.pointer_y);
    }
}

/// A pointer went up, or was canceled.
pub fn camera_pointer_up(kind: PointerKind, pointer_id: i32) {
    let s = input();
    let camera = camera_store();
    if kind == PointerKind::Touch {
        s.active_pointers.remove(&pointer_id);
        match s.active_pointers.len() {
            0 => {
                let mode = s.drag_mode;
                s.drag_mode = DragMode::None;
                camera.camera_status = CameraStatus::Idle;
                request_world_tick();
                if mode != DragMode::None {
                    tap_if_still(s, mode);
                }
            }
            1 => {
                // Already mid-gesture, so the lift is no tap.
                let remaining = *s.active_pointers.values().next().or_bug(Errno::ENOTRECOVERABLE);
                s.drag_mode = DragMode::Pan;
                s.last_x = remaining.x;
                s.last_y = remaining.y;
                s.did_drag = true;
            }
            _ => begin_pinch(s),
        }
        return;
    }
    if s.drag_mode == DragMode::None {
        return;
    }
    let mode = s.drag_mode;
    s.drag_mode = DragMode::None;
    camera.camera_status = CameraStatus::Idle;
    // A pan moves nothing once the pointer is up, so the companion would not see
    // the release until something else asked for a turn.
    request_world_tick();
    tap_if_still(s, mode);
}

pub fn camera_pointer_out() {
    input().hovering = false;
    clear_hover();
}

/// What the pointer holds changed: a hovering pointer picks again where it stands.
pub fn camera_rehover() {
    let s = input();
    if s.hovering {
        s.hover_pending = true;
    }
}

/// In CSS pixels.
pub fn camera_wheel(delta_y: f64) {
    *input().pending_wheel_delta_y.get_or_insert(0.0) += delta_y;
}

/// Each axis -1, 0 or 1, x to the right and y down.
pub fn camera_keys(x: i32, y: i32) {
    let s = input();
    let len = libm::hypot(x as f64, y as f64);
    let keys = if len > 0.0 { [x as f64 / len, y as f64 / len] } else { [0.0; 2] };
    if s.keys == [0.0; 2] {
        s.keys_at = now();
    }
    s.keys = keys;
    request_world_tick();
}

pub fn camera_input_step() {
    let s = input();
    if s.keys != [0.0; 2] {
        let at = now();
        let px = KEY_PAN_PX_PER_S * (at - s.keys_at) / 1000.0;
        s.keys_at = at;
        // A key moves the view the way it points, as dragging the ground the other way does.
        do_pan(-s.keys[0] * px, -s.keys[1] * px);
        request_world_tick();
    }
    if s.hover_pending {
        s.hover_pending = false;
        if s.drag_mode == DragMode::None {
            hover_at(s.pointer_x, s.pointer_y);
        }
    }
    if s.drag_pending {
        s.drag_pending = false;
        match s.drag_mode {
            DragMode::Pan => do_pan(s.pending_pan_dx, s.pending_pan_dy),
            DragMode::Rotate => {
                let (x, y) = (s.pending_client_x, s.pending_client_y);
                do_rotate(s, x, y);
            }
            DragMode::Pinch => update_pinch(s),
            DragMode::None => {}
        }
        s.pending_pan_dx = 0.0;
        s.pending_pan_dy = 0.0;
    }
    if let Some(delta_y) = s.pending_wheel_delta_y.take() {
        crate::control::camera::controller::wheel(delta_y);
    }
}

pub fn pinching() -> bool {
    input().drag_mode == DragMode::Pinch
}

pub fn dragging() -> bool {
    input().drag_mode != DragMode::None
}

/// Movement keys are held: the view is being walked, not dragged.
pub fn steering() -> bool {
    input().keys != [0.0; 2]
}
