// pxl-kernel/control/camera/input.rs

//! The pointer's gestures: pan, rotate, pinch/twist/tilt, wheel, hover and
//! tap. Store writes are coalesced to once a frame (`camera_input_step`).
//! What the pointer holds comes with each event; when it changes under a
//! still pointer, the shell resends the move to re-resolve hover.

use std::collections::BTreeMap;

use camera_cfg::input::{AXIS_LOCK_THRESHOLD_PX, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX, TAP_DRAG_THRESHOLD_PX, TILT_SWEEP_FRACTION};
use pxl_spec::bake::constant::{CELL, U_PER_CELL};
use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

pub use crate::abi::cmd::{ActiveTool, PointerKind};
use crate::control::camera::config::camera_cfg;
use crate::control::camera::controller::{active, Active};
use crate::control::camera::gesture::{linear, Anchor};
use crate::control::camera::ray::{ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner};
use crate::control::camera::store::{camera_store, CameraStatus, IntentPatch};
use crate::control::navigation::destination::cancel as cancel_destination;
use crate::global::Global;
use crate::layer::decor::hit::{raycast_decor, DecorStruck, Scattered};
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::geometry::{anchor_center, object_aabb};
use crate::layer::object::size::{DECOR_SIZE, DEFAULT_OBJECT_SIZE};
use crate::layer::object::store::{get_placed_object, place, place_air, place_decor, remove, OccupiedEntry};
use crate::control::navigation::walk::stand_or_raw;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RotateAxisLock {
    None,
    Tilt,
    Rotate,
}

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
    holding: Holding,
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

enum Struck {
    Object(OccupiedEntry),
    Scattered(Scattered),
}

/// The decor mesh hit wins only if closer than `hit.t`.
fn resolve_struck(hit: &Hit, origin: V3, direction: V3, map_origin: IVec) -> Option<Struck> {
    match raycast_decor(origin, direction, map_origin, hit.t) {
        Some(DecorStruck::Placed(at)) => {
            if let Some(o) = get_placed_object(at) {
                return Some(Struck::Object(o));
            }
        }
        Some(DecorStruck::Scattered(s)) => return Some(Struck::Scattered(s)),
        None => {}
    }
    resolve_hit_object(hit).map(Struck::Object)
}

/// A scattered instance is not an entry, so a placement lands as on the ground.
fn resolve_hit_target(hit: &Hit, origin: V3, direction: V3, map_origin: IVec) -> Option<OccupiedEntry> {
    match resolve_struck(hit, origin, direction, map_origin) {
        Some(Struck::Object(o)) => Some(o),
        _ => resolve_hit_object(hit),
    }
}

fn handle_tap(hovered: Pvf3) {
    let ui = input().holding;
    let object_size = ui.object_size;

    match ui.active_tool {
        ActiveTool::Pointer => {
            // Recenter where a walker would stand across the struck face, by the
            // ground's own rule, so a pan after a tap changes nothing.
            let Some(approach) = camera_store().hovered_approach else { return };
            let target = stand_or_raw(&anchor_center(&approach));
            crate::control::navigation::destination::go_to(target, crate::control::camera::controller::Motion::Smooth);
        }
        ActiveTool::Eraser => {
            match (camera_store().hovered_object_at, camera_store().hovered_scattered) {
                (Some(at), _) => remove(at),
                (None, Some(s)) => place_air(&CELL.base_of(s.cell), U_PER_CELL),
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier: the scan emits nothing for a zero material whatever its
                // sub-cell bytes, so a smaller leaf would save a bite nobody took.
                (None, None) => place_air(&hovered, U_PER_CELL),
            }
        }
        ActiveTool::Decor | ActiveTool::Object => {
            // The slot may have filled since the last hover update.
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
        if total_dx.hypot(total_dy) < AXIS_LOCK_THRESHOLD_PX {
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
    // atan2 is unstable near the pivot: drop the anchor and re-anchor later.
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

/// On the second finger down, and again when a third lifts, so the gesture never jumps.
fn begin_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    s.pinch_start_dist = (p1.x - p0.x).hypot(p1.y - p0.y);
    crate::control::camera::controller::pinch_begin(s.pinch_start_dist);
    let camera = camera_store();
    s.pinch_tilt = Anchor::new(camera.intent.polar, (p0.y + p1.y) / 2.0);
    s.pinch_twist = None;
    s.drag_mode = DragMode::Pinch;
    s.did_drag = true;
    camera.hovered_coord = None;
}

fn update_pinch(s: &mut Input) {
    let Some((p0, p1)) = pointer_pair(s) else { return };
    if s.pinch_start_dist < 1.0 {
        return;
    }
    let dist = (p1.x - p0.x).hypot(p1.y - p0.y);
    let midpoint_y = (p0.y + p1.y) / 2.0;
    crate::control::camera::controller::pinch(dist);

    let intent = camera_store().intent;
    // An unmoved control is not asked, so a zoom-only pinch writes no tilt.
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
    camera.hovered_scattered = None;
    camera.hovered_approach = None;
}

fn update_hover(s: &Input) {
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    view_ray_local(s.pointer_x, s.pointer_y, &mut origin, &mut direction);

    let ui = s.holding;
    let map_origin = local_world_origin();
    let Some(pick) = ray_pick(origin, direction) else {
        return clear_hovered_once();
    };
    let hit = pick.hit;
    let camera = camera_store();

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor) {
        camera.hovered_object_at = None;
        camera.hovered_scattered = None;
        camera.hovered_approach = None;
        let placement_size = if ui.active_tool == ActiveTool::Decor { DECOR_SIZE } else { ui.object_size };
        // A decor met in front of the pick is what the new cube goes flush against.
        let precise_box = resolve_hit_target(&hit, origin, direction, map_origin).map(|o| object_aabb(&o, map_origin));
        camera.hovered_coord = Some(world_adjacent_corner(&hit, placement_size, map_origin, precise_box.as_ref()));
        return;
    }

    let struck = resolve_struck(&hit, origin, direction, map_origin);
    camera.hovered_object_at = match &struck {
        Some(Struck::Object(o)) => Some(o.at),
        _ => None,
    };
    camera.hovered_scattered = match struck {
        Some(Struck::Scattered(s)) => Some(s),
        _ => None,
    };
    // Where a walker sent here starts from; a scattered decor is passed through.
    camera.hovered_approach = Some(match &struck {
        Some(Struck::Object(o)) => world_adjacent_corner(&hit, U_PER_CELL, map_origin, Some(&object_aabb(o, map_origin))),
        Some(Struck::Scattered(s)) => CELL.base_of(s.cell),
        None => world_adjacent_corner(&hit, U_PER_CELL, map_origin, None),
    });
    camera.hovered_coord = Some(match struck {
        Some(Struck::Object(o)) => Pvf3 { i: o.i, f: o.f },
        Some(Struck::Scattered(s)) => CELL.base_of(s.cell),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => world_struck_corner(&hit, U_PER_CELL, map_origin),
    });
}

fn tap_if_still(s: &Input, mode: DragMode) {
    if !s.did_drag && mode == DragMode::Pan {
        // A badge takes the tap before the ground under it, with the pointer tool only.
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
    clear_hovered_once();
}

/// In CSS pixels.
pub fn camera_wheel(delta_y: f64) {
    *input().pending_wheel_delta_y.get_or_insert(0.0) += delta_y;
}

pub fn camera_input_step() {
    let s = input();
    if s.hover_pending {
        s.hover_pending = false;
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

pub fn pinching() -> bool {
    input().drag_mode == DragMode::Pinch
}

pub fn dragging() -> bool {
    input().drag_mode != DragMode::None
}

/// What the hover cursor previews.
pub fn holding() -> Holding {
    input().holding
}
