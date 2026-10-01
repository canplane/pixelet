// pxl-kernel/control/camera/input.rs

//! The pointer's gestures: pan, rotate, pinch/twist/tilt, wheel dolly, hover
//! and tap. The shell forwards the raw pointer and wheel events it receives on
//! the canvas; everything they mean is decided here, and the store writes they
//! make are coalesced to once a frame (`camera_input_step`).
//!
//! What the pointer holds — the tool, and for placing, what and how big —
//! comes with each pointer event, as a modifier key does: the pick is the
//! shell's, and the kernel keeps only the last event's. When the pick changes
//! under a pointer that has not moved, the shell sends that pointer's move
//! again, as a browser synthesizes a mouse move to re-resolve hover.

use std::collections::BTreeMap;

use camera_cfg::input::{AXIS_LOCK_THRESHOLD_PX, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX, TAP_DRAG_THRESHOLD_PX, TILT_SWEEP_FRACTION};
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

pub use crate::abi::cmd::{ActiveTool, PointerKind};
use crate::control::camera::config::camera_cfg;
use crate::control::camera::controller::{active, Active};
use crate::control::camera::gesture::{linear, Anchor};
use crate::control::camera::ray::{ray_pick, resolve_hit_object, world_adjacent_corner, world_hover_pvf3, world_struck_corner};
use crate::control::camera::store::{camera_store, CameraStatus, IntentPatch};
use crate::control::navigation::destination::cancel as cancel_destination;
use crate::global::Global;
use crate::layer::decor::hit::raycast_decor;
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::geometry::{anchor_center, object_aabb, object_center};
use crate::layer::object::size::{DECOR_SIZE, DEFAULT_OBJECT_SIZE};
use crate::layer::object::store::{get_placed_object, place, place_air, place_decor, remove, OccupiedEntry};
use crate::layer::terrain::ground_snap::ground_or_raw;
use crate::shell::setting::store::settings_store;
use crate::world::access::Hit;
use crate::world::change::request_world_tick;
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view::{view, view_ray_local};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    None,
    Pan,
    Rotate,
    Pinch,
}

/// Which single axis a rotate-drag has committed to — see
/// AXIS_LOCK_THRESHOLD_PX's doc comment in config.rs for why this exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RotateAxisLock {
    None,
    Tilt,
    Rotate,
}

/// What the pointer holds, as INPUT_POINTER carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// The hotbar only ever sets this to a flat color, but the drawer picks
    /// terrain-styled cubes too.
    pub object_cube: PlaceableCube,
    pub object_size: f64,
    pub decor_id: DecorId,
}

impl Default for Holding {
    fn default() -> Holding {
        Holding {
            active_tool: ActiveTool::Pointer,
            object_cube: PlaceableCube::Block(cube_types::Block::red),
            object_size: DEFAULT_OBJECT_SIZE,
            decor_id: DecorId::tree,
        }
    }
}

/// The canvas's client rect, as the shell measured it.
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
    /// What the last pointer event held.
    holding: Holding,
    drag_mode: DragMode,
    did_drag: bool,
    pointer_x: f64,
    pointer_y: f64,
    rect: Option<Rect>,
    last_x: f64,
    last_y: f64,
    // The pointerdown position for the current gesture — total displacement
    // from here, not the per-event dx/dy, decides a tap from a drag.
    tap_start_x: f64,
    tap_start_y: f64,
    hover_pending: bool,

    // ---- drag-continuation coalescing ----
    // Raw pointermove fires far more often than the render loop, and every pan,
    // rotate and pinch writes a store whose readers fan out. Profiled: without
    // this, pointer moves dominated a whole trace's CPU time during a drag. The
    // tap-vs-drag threshold still updates on every raw event; only the store
    // writes wait for the frame, coalescing however many events arrived.
    drag_pending: bool,
    pending_pan_dx: f64,
    pending_pan_dy: f64,
    pending_client_x: f64,
    pending_client_y: f64,
    // Wheel/trackpad-pinch zoom — the more likely culprit for LOD-storm lag,
    // since a distance change can cross every visible chunk's threshold at once.
    // Summing deltaY before one exp() is exact: exp(a)*exp(b) == exp(a+b).
    pending_wheel_delta_y: Option<f64>,

    // ---- rotate-drag ----
    /// The heading against the cursor's angle about screen center, once a
    /// sample far enough from it to read has anchored it.
    rotate_turn: Option<Anchor>,
    /// The tilt against the cursor's height: a tilt is the drag's vertical
    /// travel from there, as a pinch's is from its fingers' midpoint.
    rotate_tilt: Anchor,
    rotate_drag_start_x: f64,
    rotate_drag_start_y: f64,
    rotate_axis_lock: RotateAxisLock,

    // ---- multi-touch ----
    active_pointers: BTreeMap<i32, PointerPos>,
    pinch_start_dist: f64,
    /// The tilt against the fingers' midpoint.
    pinch_tilt: Anchor,
    /// The heading against the fingers' angle, once they are far enough
    /// apart to read it.
    pinch_twist: Option<Anchor>,
}

static INPUT: Global<Input> = Global::new();

fn input() -> &'static mut Input {
    INPUT.get(|| Input {
        holding: Holding::default(),
        drag_mode: DragMode::None,
        did_drag: false,
        pointer_x: 0.0,
        pointer_y: 0.0,
        rect: None,
        last_x: 0.0,
        last_y: 0.0,
        tap_start_x: 0.0,
        tap_start_y: 0.0,
        hover_pending: false,
        drag_pending: false,
        pending_pan_dx: 0.0,
        pending_pan_dy: 0.0,
        pending_client_x: 0.0,
        pending_client_y: 0.0,
        pending_wheel_delta_y: None,
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

/// Which entry `hit` actually struck, decor included — the placed decor's mesh
/// hit first, bounded by hit.t: decor sitting on solid ground can only be the
/// right answer when it's genuinely closer along the ray than whatever the
/// pick met, falling back to the object the pick met.
fn resolve_hit_target(hit: &Hit, origin: V3, direction: V3, map_origin: IVec) -> Option<OccupiedEntry> {
    if let Some(decor) = raycast_decor(origin, direction, map_origin, hit.t).and_then(get_placed_object) {
        return Some(decor);
    }
    resolve_hit_object(hit)
}

/// What a non-drag tap on the terrain means — recenter the map (pointer tool),
/// place a cube (object tool), or remove/dig with the eraser tool (an existing
/// placed object if there's one under the cursor, the terrain's own real surface
/// otherwise).
fn handle_tap(hovered: Pvf3) {
    let ui = input().holding;
    let object_size = ui.object_size;

    match ui.active_tool {
        ActiveTool::Pointer => {
            // Recenter on the center of the cube the tap names: an object's own
            // center, or the center of the cube standing on the struck ground,
            // pivoted by the ground's own rule — what every pan, the bootstrap
            // and follow mode set too, so a pan after a terrain tap changes
            // nothing.
            let hovered_object = camera_store().hovered_object_at.and_then(get_placed_object);
            let target = match hovered_object {
                Some(o) => object_center(&o, local_world_origin()),
                None => ground_or_raw(&anchor_center(&hovered)),
            };
            crate::control::navigation::destination::go_to(target, crate::control::camera::controller::Motion::Smooth);
        }
        ActiveTool::Eraser => {
            // Targets the object the raycast actually struck, not a grid-key
            // lookup — correct for any size, any face.
            match camera_store().hovered_object_at {
                Some(at) => remove(at),
                // Nothing placed here — dig into the terrain's own real surface,
                // once the user has opted into that. AIR, not VOID: real terrain
                // has no object-layer leaf of its own to delete, so an explicit
                // "nothing here on purpose" leaf is the only way to override it.
                //
                // A cell, never the size tier. A zero material is a voxel the
                // scan emits nothing for whatever its sub-cell bytes say
                // (pxl-render mesh/voxel_scan.rs), so a smaller leaf would still
                // take the cell whole while recording a bite nobody took — and
                // the store is saved, so that number outlives the session.
                None if settings_store().terrain_carving => place_air(&hovered, U_PER_CELL),
                None => {}
            }
        }
        ActiveTool::Decor | ActiveTool::Object => {
            // Still guard against the candidate slot being occupied since the
            // last hover update.
            if ui.active_tool == ActiveTool::Decor {
                if !is_placement_blocked(PlacementTool::Decor, &hovered, object_size) {
                    place_decor(&hovered, DECOR_SIZE, ui.decor_id);
                }
            } else if !is_placement_blocked(PlacementTool::Object, &hovered, object_size) {
                place(&hovered, object_size, ui.object_cube, settings_store().structures);
            }
        }
    }
}

/// Radians of pitch per pixel of vertical drag. Negative because dy grows
/// downward and dragging up is the natural "tilt toward the horizon".
fn tilt_gain() -> f64 {
    // The whole tilt range is the look's θ, straight down to straight up
    // (orbit.rs `look_domain`).
    -core::f64::consts::PI / (TILT_SWEEP_FRACTION * view().height)
}

/// The canvas's rect moved (a resize, a scroll).
pub fn camera_input_rect(rect: Rect) {
    input().rect = Some(rect);
}

fn update_pointer(s: &mut Input, client_x: f64, client_y: f64) {
    let Some(rect) = s.rect else { return };
    s.pointer_x = ((client_x - rect.left) / rect.width) * 2.0 - 1.0;
    s.pointer_y = -((client_y - rect.top) / rect.height) * 2.0 + 1.0;
}

/// Shared pan implementation — driven by either a mouse left-drag or a single
/// touch: the active controller's own. A pan puts the map center by hand, so
/// a destination still waiting for its ground goes.
fn do_pan(dx: f64, dy: f64) {
    match active() {
        Active::Orbit => {
            cancel_destination();
            crate::control::camera::orbit::pan(dx, dy);
        }
        Active::Free => {
            cancel_destination();
            crate::control::camera::free::pan(dx, dy);
        }
        Active::Tracked => {}
    }
}

/// Azimuth tracks the cursor's angle around screen center since drag-start
/// (arcball-style), not a fixed px/degree rate — screen center is the fixed
/// pivot since the camera always looks at map_center. The heading turns
/// against the cursor's angle delta. Polar (tilt) is a fixed rate, likewise
/// from the drag's start rather than stepped from the store's last value, so
/// the drag's whole travel is the tilt and the view comes back where it began
/// when the hand does. The azimuth
/// anchor is captured lazily, once the cursor clears MIN_ROTATE_RADIUS_PX from
/// center, since atan2 is degenerate at the pivot. The gesture commits to
/// whichever axis dominates past AXIS_LOCK_THRESHOLD_PX and keeps it: an
/// off-center "straight down" drag still sweeps some angle around center,
/// which read as an unrelated spin once polar saturated.
fn do_rotate(s: &mut Input, client_x: f64, client_y: f64) {
    if s.rotate_axis_lock == RotateAxisLock::None {
        let total_dx = client_x - s.rotate_drag_start_x;
        let total_dy = client_y - s.rotate_drag_start_y;
        if total_dx.hypot(total_dy) < AXIS_LOCK_THRESHOLD_PX {
            return;
        }
        s.rotate_axis_lock = if total_dy.abs() > total_dx.abs() { RotateAxisLock::Tilt } else { RotateAxisLock::Rotate };
    }

    if s.rotate_axis_lock == RotateAxisLock::Tilt {
        // tilt_lock only gates this write — an experiment toggle for a
        // fixed-tilt UX, not yet a decided product change.
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
    // atan2's angle is numerically unstable close to the pivot: freeze
    // azimuth and drop the anchor below that radius, re-anchoring from the
    // next reliable sample.
    if dx.hypot(dyc) < MIN_ROTATE_RADIUS_PX {
        s.rotate_turn = None;
        return;
    }
    let angle = dyc.atan2(dx);
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

/// (Re)captures the two-finger baseline: on the 2nd finger touching down, and
/// again when a 3rd+ finger lifts back to exactly 2, so the gesture never jumps.
fn begin_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    s.pinch_start_dist = (p1.x - p0.x).hypot(p1.y - p0.y);
    crate::control::camera::controller::pinch_begin(s.pinch_start_dist);
    let camera = camera_store();
    s.pinch_tilt = Anchor::new(camera.intent.polar, (p0.y + p1.y) / 2.0);
    // The twist angle is anchored lazily once the fingers are far enough
    // apart for it to be reliable.
    s.pinch_twist = None;
    s.drag_mode = DragMode::Pinch;
    s.did_drag = true;
    camera.hovered_coord = None;
}

/// Pinch (zoom), twist (azimuth), and two-finger vertical drag (tilt) — the
/// standard mobile map gesture set.
fn update_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    if s.pinch_start_dist < 1.0 {
        return;
    }
    let dist = (p1.x - p0.x).hypot(p1.y - p0.y);
    let midpoint_y = (p0.y + p1.y) / 2.0;
    // The fingers' spread is the active controller's zoom; their midpoint
    // and their turn are the look's.
    crate::control::camera::controller::pinch(dist);

    let intent = camera_store().intent;
    // A control the fingers did not move is not asked: a pinch that only
    // zooms writes no tilt, which the ground could hold to where it is.
    let polar = s.pinch_tilt.ask(midpoint_y, linear(tilt_gain()));
    let polar = (!settings_store().tilt_lock && polar != intent.polar).then_some(polar);
    let angle = (p1.y - p0.y).atan2(p1.x - p0.x);
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

/// A pointer went down on the canvas.
pub fn camera_pointer_down(kind: PointerKind, pointer_id: i32, button: i32, client_x: f64, client_y: f64, holding: Holding) {
    let s = input();
    s.holding = holding;
    let camera = camera_store();
    if kind == PointerKind::Touch {
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
            // 3rd+ finger: keep the current 2-finger gesture as-is.
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
        // The turn's anchor is captured lazily by do_rotate on the first
        // reliable sample — the down position may be within
        // MIN_ROTATE_RADIUS_PX.
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

/// A pointer moved.
pub fn camera_pointer_move(kind: PointerKind, pointer_id: i32, client_x: f64, client_y: f64, holding: Holding) {
    let s = input();
    s.holding = holding;
    if kind == PointerKind::Touch {
        if let Some(pt) = s.active_pointers.get_mut(&pointer_id) {
            *pt = PointerPos { x: client_x, y: client_y };
        }
        if s.drag_mode == DragMode::Pinch && s.active_pointers.len() >= 2 {
            s.drag_pending = true;
            return;
        }
        if s.drag_mode == DragMode::Pan && s.active_pointers.len() == 1 {
            if (client_x - s.tap_start_x).hypot(client_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
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
        // Raycasting is expensive and a raw pointermove can fire far more often
        // than the render loop: once a frame.
        s.hover_pending = true;
        return;
    }

    if (client_x - s.tap_start_x).hypot(client_y - s.tap_start_y) > TAP_DRAG_THRESHOLD_PX {
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

fn clear_hovered_once() {
    let camera = camera_store();
    if camera.hovered_coord.is_some() {
        camera.hovered_coord = None;
    }
    if camera.hovered_object_at.is_some() {
        camera.hovered_object_at = None;
    }
}

fn update_hover(s: &Input) {
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    view_ray_local(s.pointer_x, s.pointer_y, &mut origin, &mut direction);

    let ui = s.holding;
    let map_origin = local_world_origin();
    // One pick — terrain (top or a wall) and any placed object all come back
    // as the same Hit.
    let Some(pick) = ray_pick(origin, direction) else {
        return clear_hovered_once();
    };
    let hit = pick.hit;
    let camera = camera_store();

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor) {
        // The one tool group that wants an ADJACENT empty slot. Decor always
        // places at its own fixed DECOR_SIZE.
        camera.hovered_object_at = None;
        let placement_size = if ui.active_tool == ActiveTool::Decor { DECOR_SIZE } else { ui.object_size };
        // The box of the entry the hit resolves to: a decor met in front of
        // what the pick struck is what the new cube goes flush against.
        let precise_box = resolve_hit_target(&hit, origin, direction, map_origin).map(|o| object_aabb(&o, map_origin));
        camera.hovered_coord = Some(world_adjacent_corner(&hit, placement_size, map_origin, precise_box.as_ref()));
        return;
    }

    // Pointer (recenter on it) and eraser (remove/dig it) both target the struck
    // thing itself.
    let struck = resolve_hit_target(&hit, origin, direction, map_origin);
    camera.hovered_object_at = struck.as_ref().map(|o| o.at);
    if let Some(o) = struck {
        camera.hovered_coord = Some(Pvf3 { i: o.i, f: o.f });
    } else if ui.active_tool == ActiveTool::Eraser && settings_store().terrain_carving {
        // Bare terrain under the eraser wants the struck voxel's OWN corner
        // (real, possibly dug-into height included), not the baked-elevation
        // "flush on top" slot. On the cell's grid and not the tier's, because
        // that is the size the carve is written at.
        camera.hovered_coord = Some(world_struck_corner(&hit, U_PER_CELL, map_origin));
    } else {
        match world_hover_pvf3(&hit, map_origin) {
            Some(grounded) => camera.hovered_coord = Some(grounded),
            None => clear_hovered_once(),
        }
    }
}

fn tap_if_still(s: &Input, mode: DragMode) {
    if !s.did_drag && mode == DragMode::Pan {
        // A badge floats over the ground it marks, so it takes the tap before
        // the ground under it does — with the pointer only: with a cube or the
        // eraser in hand, a tap is meant for the ground and the objects on it.
        if s.holding.active_tool == ActiveTool::Pointer && crate::layer::poi::placement::tap(s.pointer_x, s.pointer_y) {
            return;
        }
        update_hover(s);
        if let Some(hovered) = camera_store().hovered_coord {
            handle_tap(hovered);
        }
    }
}

/// A pointer went up, or was canceled.
pub fn camera_pointer_up(kind: PointerKind, pointer_id: i32, holding: Holding) {
    let s = input();
    s.holding = holding;
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
                // Dropped from 2 fingers to 1 — resume single-finger pan
                // seamlessly; already mid-gesture, so the lift is no tap.
                let remaining = *s.active_pointers.values().next().or_bug(Errno::ENOTRECOVERABLE);
                s.drag_mode = DragMode::Pan;
                s.last_x = remaining.x;
                s.last_y = remaining.y;
                s.did_drag = true;
            }
            // Dropped from 3+ fingers to exactly 2 — recalibrate the baseline.
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
    // The turn the companion reads the release in (layer/character/companion.rs):
    // a pan moves nothing on its own once the pointer is up, so without this
    // the release would not be seen until something else asked for a turn.
    request_world_tick();
    tap_if_still(s, mode);
}

/// The pointer left the canvas.
pub fn camera_pointer_out() {
    clear_hovered_once();
}

/// A wheel event's turn in CSS pixels.
pub fn camera_wheel(delta_y: f64) {
    *input().pending_wheel_delta_y.get_or_insert(0.0) += delta_y;
}

/// The input's turn, once a frame: the coalesced hover, drag and wheel.
pub fn camera_input_step() {
    let s = input();
    if s.hover_pending {
        s.hover_pending = false;
        // The pointer may have started dragging since the move.
        if s.drag_mode == DragMode::None {
            update_hover(s);
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

/// Two fingers are on the canvas, zooming, tilting or twisting it.
pub fn pinching() -> bool {
    input().drag_mode == DragMode::Pinch
}

/// A finger or a button is on the canvas, moving the map in any way.
pub fn dragging() -> bool {
    input().drag_mode != DragMode::None
}

/// What the last pointer event held: what the hover cursor previews.
pub fn holding() -> Holding {
    input().holding
}
