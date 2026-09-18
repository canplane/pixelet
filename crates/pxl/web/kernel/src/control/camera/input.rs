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

use camera_cfg::input::{AXIS_LOCK_THRESHOLD_PX, DOLLY_GAIN, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX, TAP_DRAG_THRESHOLD_PX, TILT_SWEEP_FRACTION};
use camera_cfg::projection::FOV;
use camera_cfg::view::{MAX_POLAR, MIN_POLAR};
use pxl_spec::cell::{cube_types, DecorId, PlaceableCube};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pvf3::pvf3::PVF3;
use xpute_core::kit::mat4::Vec3;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::ray::{ray_pick, resolve_hit_object, world_adjacent_corner, world_hover_pvf3, world_struck_corner};
use crate::control::camera::store::{camera_store, CameraStatus, ViewPatch};
use crate::control::camera::terrain_gate::{allowed_pan, ViewTriple};
use crate::layer::decor::hit::raycast_decor;
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::geometry::{anchor_center, object_aabb, object_center};
use crate::layer::object::size::{DECOR_SIZE, DEFAULT_OBJECT_SIZE};
use crate::layer::object::store::{get_placed_object, place, place_air, place_decor, remove, OccupiedEntry};
use crate::layer::terrain::elevation::ground_or_raw;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;
use crate::sys::global::Global;
use crate::world::chunk::raycast::WorldHit;
use crate::world::frame::coord::get_local_world_origin;
use crate::world::frame::view::{view, view_ray_local};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragMode {
    None,
    Pan,
    Rotate,
    Pinch,
}

/// Which single axis a rotate-drag has committed to — see
/// AXIS_LOCK_THRESHOLD_PX's doc comment in config.json for why this exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RotateAxisLock {
    None,
    Tilt,
    Rotate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveTool {
    #[default]
    Pointer,
    Object,
    Decor,
    Eraser,
}

/// What the pointer holds, as INPUT_POINTER carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// The hotbar only ever sets this to a flat colour, but the drawer picks
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
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
    rotate_start_angle: f64,
    rotate_start_azimuth: f64,
    /// The pitch when the drag began: a tilt is the drag's whole vertical
    /// travel from there, as a pinch's is from its fingers' first midpoint.
    rotate_start_polar: f64,
    rotate_anchored: bool,
    rotate_drag_start_x: f64,
    rotate_drag_start_y: f64,
    rotate_axis_lock: RotateAxisLock,

    // ---- multi-touch ----
    active_pointers: BTreeMap<i32, PointerPos>,
    pinch_start_dist: f64,
    pinch_start_angle: f64,
    pinch_start_midpoint_y: f64,
    pinch_start_view_dist: f64,
    pinch_start_azimuth: f64,
    pinch_start_polar: f64,
    twist_anchored: bool,
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
        rotate_start_angle: 0.0,
        rotate_start_azimuth: 0.0,
        rotate_start_polar: 0.0,
        rotate_anchored: false,
        rotate_drag_start_x: 0.0,
        rotate_drag_start_y: 0.0,
        rotate_axis_lock: RotateAxisLock::None,
        active_pointers: BTreeMap::new(),
        pinch_start_dist: 0.0,
        pinch_start_angle: 0.0,
        pinch_start_midpoint_y: 0.0,
        pinch_start_view_dist: 0.0,
        pinch_start_azimuth: 0.0,
        pinch_start_polar: 0.0,
        twist_anchored: false,
    })
}

/// Which entry `hit` actually struck, decor included — the placed decor's mesh
/// raycast first, bounded by hit.t: decor sitting on solid ground can only be
/// the right answer when it's genuinely closer along the ray than whatever the
/// voxel raycast found, falling back to ray.rs's solid-object AABB resolution.
fn resolve_hit_target(hit: &WorldHit, origin: Vec3, direction: Vec3, map_origin: IVec) -> Option<OccupiedEntry> {
    if let Some(decor) = raycast_decor(origin, direction, map_origin, hit.t).and_then(get_placed_object) {
        return Some(decor);
    }
    resolve_hit_object(hit, map_origin)
}

/// What a non-drag tap on the terrain means — recenter the map (pointer tool),
/// place a cube (object tool), or remove/dig with the eraser tool (an existing
/// placed object if there's one under the cursor, the terrain's own real surface
/// otherwise).
fn handle_tap(hovered: PVF3) {
    let ui = input().holding;
    let object_size = ui.object_size;

    match ui.active_tool {
        ActiveTool::Pointer => {
            // Recenter on the centre of the cube the tap names: an object's own
            // centre, or the centre of the cube standing on the struck ground,
            // pivoted by the ground's own rule — what every pan, the bootstrap
            // and follow mode set too, so a pan after a terrain tap changes
            // nothing.
            let hovered_object = camera_store().hovered_object_at.and_then(get_placed_object);
            let target = match hovered_object {
                Some(o) => object_center(&o, get_local_world_origin(map_store().map_center.as_ref())),
                None => ground_or_raw(&anchor_center(&hovered)),
            };
            map_store().map_center = Some(target);
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
                None if settings_store().enable_terrain_carving => place_air(&hovered, object_size),
                None => {}
            }
        }
        ActiveTool::Decor | ActiveTool::Object => {
            // Still guard against the candidate slot being occupied since the
            // last hover update.
            let map_origin = get_local_world_origin(map_store().map_center.as_ref());
            if ui.active_tool == ActiveTool::Decor {
                if !is_placement_blocked(PlacementTool::Decor, &hovered, object_size, map_origin) {
                    place_decor(&hovered, DECOR_SIZE, ui.decor_id);
                }
            } else if !is_placement_blocked(PlacementTool::Object, &hovered, object_size, map_origin) {
                place(&hovered, object_size, ui.object_cube, settings_store().show_structures);
            }
        }
    }
}

/// Radians of pitch per pixel of vertical drag. Negative because dy grows
/// downward and dragging up is the natural "tilt toward the horizon".
fn tilt_gain() -> f64 {
    -(MAX_POLAR - MIN_POLAR) / (TILT_SWEEP_FRACTION * view().height)
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

/// Shared pan implementation — driven by either a mouse left-drag or a single touch.
fn do_pan(dx: f64, dy: f64) {
    let view_state = camera_store().view_state;
    let Some(map_center) = map_store().map_center else { return };

    let view_h = 2.0 * view_state.dist * (FOV * core::f64::consts::PI / 360.0).tan();
    let units_per_px = view_h / view().height;
    let lx = -dx * units_per_px;
    let ly = dy * units_per_px;
    let azimuth = view().azimuth;
    let wx = lx * azimuth.cos() + ly * azimuth.sin();
    let wy = -lx * azimuth.sin() + ly * azimuth.cos();

    let mut center = [0.0; 3];
    projection::unproject(&mut center, map_center.i, map_center.f);
    let mut next_center = PVF3::default();
    projection::project(&mut next_center, center[0] + wx, center[1] + wy, center[2]);

    // ground_or_raw shares the exact same "zero Z, then sample real terrain
    // height" step every map_center-setting interaction that respects
    // ground_snap goes through.
    let desired = if settings_store().ground_snap { ground_or_raw(&next_center) } else { next_center };
    // Gated after the snap, not before: the snap moves the target's Z, and the
    // eye rides on it. Panning is the one control that moves the target.
    map_store().map_center = Some(allowed_pan(
        ViewTriple {
            dist: view_state.dist,
            polar: view_state.polar,
            azimuth: view_state.azimuth,
        },
        &desired,
    ));
}

/// Azimuth tracks the cursor's angle around screen center since drag-start
/// (arcball-style), not a fixed px/degree rate — screen center is the fixed
/// pivot since the camera always looks at map_center. World rotates opposite the
/// cursor's angle delta. Polar (tilt) is a fixed rate, but likewise from the
/// drag's start, not stepped from the store's last value: the store rounds
/// polar to 0.01 rad, and a frame's worth of a slow drag is under half a step,
/// which rounded back to nothing — a slow tilt did not move. The azimuth
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

    let camera = camera_store();
    let view_state = camera.view_state;

    if s.rotate_axis_lock == RotateAxisLock::Tilt {
        // lock_camera_tilt only gates this write — an experiment toggle for a
        // fixed-tilt UX, not yet a decided product change.
        if !settings_store().lock_camera_tilt {
            camera.set_view_state(ViewPatch {
                polar: Some(s.rotate_start_polar + (client_y - s.rotate_drag_start_y) * tilt_gain()),
                ..Default::default()
            });
        }
        return;
    }

    let mut next_azimuth = view_state.azimuth;
    if let Some(rect) = s.rect {
        let dx = client_x - (rect.left + rect.width / 2.0);
        let dyc = client_y - (rect.top + rect.height / 2.0);
        // atan2's angle is numerically unstable close to the pivot: freeze
        // azimuth and drop the anchor below that radius, re-anchoring from the
        // next reliable sample.
        if dx.hypot(dyc) >= MIN_ROTATE_RADIUS_PX {
            let cur_angle = dyc.atan2(dx);
            if !s.rotate_anchored {
                s.rotate_start_angle = cur_angle;
                s.rotate_start_azimuth = view_state.azimuth;
                s.rotate_anchored = true;
            } else {
                next_azimuth = s.rotate_start_azimuth - (cur_angle - s.rotate_start_angle);
            }
        } else {
            s.rotate_anchored = false;
        }
    }
    camera.set_view_state(ViewPatch {
        azimuth: Some(next_azimuth),
        ..Default::default()
    });
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
    s.pinch_start_midpoint_y = (p0.y + p1.y) / 2.0;
    // The twist angle is re-anchored lazily once the fingers are far enough
    // apart for it to be reliable.
    s.twist_anchored = false;
    let camera = camera_store();
    s.pinch_start_view_dist = camera.view_state.dist;
    s.pinch_start_azimuth = camera.view_state.azimuth;
    s.pinch_start_polar = camera.view_state.polar;
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
    // Fingers spreading apart zoom in.
    let next_dist = s.pinch_start_view_dist * (s.pinch_start_dist / dist);
    let next_polar = s.pinch_start_polar + (midpoint_y - s.pinch_start_midpoint_y) * tilt_gain();

    let camera = camera_store();
    let mut next_azimuth = s.pinch_start_azimuth;
    if dist >= MIN_TWIST_DIST_PX {
        let angle = (p1.y - p0.y).atan2(p1.x - p0.x);
        if !s.twist_anchored {
            s.pinch_start_angle = angle;
            s.pinch_start_azimuth = camera.view_state.azimuth;
            s.twist_anchored = true;
            next_azimuth = s.pinch_start_azimuth;
        } else {
            next_azimuth = s.pinch_start_azimuth - (angle - s.pinch_start_angle);
        }
    } else {
        s.twist_anchored = false;
    }
    camera.set_view_state(ViewPatch {
        dist: Some(next_dist),
        azimuth: Some(next_azimuth),
        polar: (!settings_store().lock_camera_tilt).then_some(next_polar),
    });
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
        // The anchor is captured lazily by do_rotate on the first reliable
        // sample — the down position may be within MIN_ROTATE_RADIUS_PX.
        s.rotate_anchored = false;
        s.rotate_axis_lock = RotateAxisLock::None;
        s.rotate_drag_start_x = client_x;
        s.rotate_drag_start_y = client_y;
        s.rotate_start_polar = camera.view_state.polar;
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
    let Some(map_center) = map_store().map_center else {
        return clear_hovered_once();
    };
    let mut origin = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut direction = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    view_ray_local(s.pointer_x, s.pointer_y, &mut origin, &mut direction);

    let ui = s.holding;
    let map_origin = get_local_world_origin(Some(&map_center));
    // One unified raycast — terrain (top or a wall) and any placed object all
    // come back as the same WorldHit, all boxes in the same voxel field.
    let Some(pick) = ray_pick(origin, direction, Some(&map_center)) else {
        return clear_hovered_once();
    };
    let hit = pick.hit;
    let camera = camera_store();

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor) {
        // The one tool group that wants an ADJACENT empty slot. Decor always
        // places at its own fixed DECOR_SIZE.
        camera.hovered_object_at = None;
        let placement_size = if ui.active_tool == ActiveTool::Decor { DECOR_SIZE } else { ui.object_size };
        // A real struck entry's exact stored AABB, not the hit's own box — that
        // comes back through the u8-packed sub_cell payload, accurate to only
        // ~1/510 of a cell, which let stacked sub-cell placements drift.
        let precise_box = resolve_hit_target(&hit, origin, direction, map_origin).map(|o| object_aabb(&o, map_origin));
        camera.hovered_coord = Some(world_adjacent_corner(&hit, placement_size, map_origin, precise_box.as_ref()));
        return;
    }

    // Pointer (recenter on it) and eraser (remove/dig it) both target the struck
    // thing itself.
    let struck = resolve_hit_target(&hit, origin, direction, map_origin);
    camera.hovered_object_at = struck.as_ref().map(|o| o.at);
    if let Some(o) = struck {
        camera.hovered_coord = Some(PVF3 { i: o.i, f: o.f });
    } else if ui.active_tool == ActiveTool::Eraser && settings_store().enable_terrain_carving {
        // Bare terrain under the eraser wants the struck voxel's OWN corner
        // (real, possibly dug-into height included), not the baked-elevation
        // "flush on top" slot.
        camera.hovered_coord = Some(world_struck_corner(&hit, ui.object_size, map_origin));
    } else {
        match world_hover_pvf3(&hit, map_origin) {
            Some(grounded) => camera.hovered_coord = Some(grounded),
            None => clear_hovered_once(),
        }
    }
}

fn tap_if_still(s: &Input, mode: DragMode) {
    if !s.did_drag && mode == DragMode::Pan {
        update_hover(s);
        if let Some(hovered) = camera_store().hovered_coord {
            handle_tap(hovered);
        }
    }
}

/// A pointer went up, or was cancelled.
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
                if mode != DragMode::None {
                    tap_if_still(s, mode);
                }
            }
            1 => {
                // Dropped from 2 fingers to 1 — resume single-finger pan
                // seamlessly; already mid-gesture, so the lift is no tap.
                let remaining = *s.active_pointers.values().next().expect("one pointer");
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
    tap_if_still(s, mode);
}

/// The pointer left the canvas.
pub fn camera_pointer_out() {
    clear_hovered_once();
}

/// A wheel event's vertical delta.
pub fn camera_wheel(delta_y: f64) {
    let s = input();
    *s.pending_wheel_delta_y.get_or_insert(0.0) += delta_y;
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
        let camera = camera_store();
        camera.set_view_state(ViewPatch {
            dist: Some(camera.view_state.dist * (delta_y * DOLLY_GAIN).exp()),
            ..Default::default()
        });
    }
}

/// What the last pointer event held: what the hover cursor previews.
pub fn holding() -> Holding {
    input().holding
}
