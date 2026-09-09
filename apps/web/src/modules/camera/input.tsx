// @pxl/web/modules/camera/input.tsx

import { useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";

import { type IVec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useFrameRefs } from "@/modules/world/frame/context.tsx";
import { ground_or_raw } from "@/modules/world/terrain/elevation.ts";
import { get_local_world_origin } from "@/modules/world/frame/coord.ts";
import { is_placement_blocked } from "@/modules/world/objects/collision.ts";
import { anchor_center, object_aabb, object_center } from "@/modules/world/objects/geometry.ts";
import { DECOR_SIZE } from "@/modules/world/objects/size.ts";
import { get_placed_object, type OccupiedEntry, place, place_air, place_decor, remove } from "@/modules/world/objects/store.ts";
import { raycast_decor } from "@/modules/world/decor/decor_hit.ts";
import { ray_pick, resolve_hit_object, world_adjacent_corner, world_hover_pvf3, world_struck_corner, type WorldHit } from "./ray.ts";
import { CameraStatus, useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";
import { allowed_pan } from "./terrain_gate.ts";

const { FOV } = camera_cfg.projection;
const { TILT_SWEEP_FRACTION, DOLLY_GAIN, MIN_ROTATE_RADIUS_PX, MIN_TWIST_DIST_PX, AXIS_LOCK_THRESHOLD_PX, TAP_DRAG_THRESHOLD_PX } = camera_cfg.input;
const { MIN_POLAR, MAX_POLAR } = camera_cfg.view;

const enum DragMode {
  NONE = "none",
  PAN = "pan",
  ROTATE = "rotate",
  PINCH = "pinch",
}

/** Which single axis a rotate-drag (do_rotate) has committed to — see
 * AXIS_LOCK_THRESHOLD_PX's doc comment in config.ts for why this exists. */
const enum RotateAxisLock {
  NONE = "none",
  TILT = "tilt",
  ROTATE = "rotate",
}

type PointerPos = { x: f64; y: f64 };

const $ray = new THREE.Ray();
const $pointer = new THREE.Vector2();

/**
 * Which OccupiedEntry `hit` actually struck, decor included — tries a real
 * mesh raycast against baked decor first (decor_hit.ts's raycast_decor,
 * bounded by hit.t: decor sitting on solid ground can only be the right
 * answer when it's genuinely closer along the ray than whatever the
 * abstract voxel raycast already found there), falling back to
 * ray.ts's own solid-object AABB resolution otherwise. Split out of
 * ray.ts itself because only this module holds the live THREE.Raycaster
 * (`raycaster`, from useThree()) a real mesh intersection test needs —
 * ray.ts's own raycast is deliberately THREE-free (chunk_raycast.ts's own
 * doc comment), so wiring a scene-graph raycast into it would mean
 * threading a THREE.Raycaster through a module that otherwise never
 * imports three's runtime types at all.
 */
function resolve_hit_target(raycaster: THREE.Raycaster, hit: WorldHit, map_origin: IVec): OccupiedEntry | null {
  const decor_id = raycast_decor(raycaster, hit.t);
  if (decor_id) {
    const decor = get_placed_object(decor_id);
    if (decor) return decor;
  }
  return resolve_hit_object(hit, map_origin);
}

/**
 * What a non-drag tap on the terrain means — recenter the map (pointer
 * tool), place a cube (object tool), or remove/dig with the eraser tool
 * (an existing placed object if there's one under the cursor, the terrain's
 * own real surface otherwise). Shared by both tap-detection sites below
 * (mouse pointerup, touch's drop-to-zero-fingers branch).
 */
function handle_tap(hovered: PVF3): void {
  const { activeTool, object_color_id, object_size, decor_id } = useUiStore.getState();

  if (activeTool === "pointer") {
    // Recenter on the middle of whatever the cursor is on — the exact point
    // the hover-cursor ghost box is drawn centered on, so "tap the
    // highlighted box, the camera goes to that box". hovered/hovered_coord
    // always stores a cell's (min-x, min-y, min-z) corner (see camera/store.ts
    // and update_hover() below); an object hit resolves it via object_center,
    // a bare-terrain hit via anchor_center, and both are the same symmetric
    // corner + half a cell on all three axes. Before, the object branch
    // centered but terrain recentered on the corner.
    //
    // Known consequence: do_pan's ground_snap (elevation.ts's ground_or_raw)
    // re-grounds Z to the bare terrain surface on the next pan step, so the
    // first drag after this nudges the camera target by up to half a cell
    // (terrain) or an object's full height above ground (a stacked cube).
    // Not fixed here: the camera's own terrain-only ground-snap and the
    // companion's object-aware surface query (surface_height.ts) are
    // deliberately kept separate for now — reconciling them is a larger
    // design call, not a tweak to this line.
    const { hovered_object_id } = useCameraStore.getState();
    const hovered_object = hovered_object_id ? get_placed_object(hovered_object_id) : null;
    const target = hovered_object ? object_center(hovered_object, get_local_world_origin(useMapStore.getState().map_center)) : anchor_center(hovered);

    useMapStore.getState().set_map_center(target);
    return;
  }

  if (activeTool === "eraser") {
    // targets the object the raycast actually struck (see update_hover()),
    // not a grid-key lookup — correct for any size, any face.
    const { hovered_object_id } = useCameraStore.getState();
    if (hovered_object_id) {
      remove(hovered_object_id);
    } else if (useSettingsStore.getState().enable_terrain_carving) {
      // Nothing placed here — dig into the terrain's own real surface
      // instead of no-op'ing, but only once the user has actually opted
      // into that (settings/slices/camera_slice.ts's enable_terrain_carving
      // — off by default, this is real baked map data, not a scratch
      // canvas). AIR, not VOID: unlike an ordinary placed object, real
      // terrain has no object-layer leaf of its own to delete, so an
      // explicit "nothing here on purpose" leaf is the only way to override
      // it. `hovered` is already the struck voxel's own real corner in this
      // branch (update_hover()'s world_struck_corner case) — real height
      // included, so erasing the same bare spot again digs one voxel deeper
      // each time: the next raycast simply hits whatever's now topmost,
      // never a cached, never-updated bake height.
      place_air(hovered.i, hovered.f, object_size);
    }
    return;
  }

  // activeTool === "object" | "decor" — `hovered` is already either a
  // terrain corner or, when the cursor is over an existing object, the
  // corner flush against whichever face was struck (adjacent_corner() in
  // update_hover()). Still guard against that candidate slot itself already
  // being occupied (e.g. something else placed in the same spot since the
  // last hover update).
  const map_center = useMapStore.getState().map_center;
  const map_origin = get_local_world_origin(map_center);

  if (activeTool === "decor") {
    if (!is_placement_blocked("decor", hovered, object_size, map_origin)) {
      place_decor(hovered.i, hovered.f, DECOR_SIZE, decor_id);
    }
    return;
  }

  if (!is_placement_blocked("object", hovered, object_size, map_origin)) {
    place(hovered.i, hovered.f, object_size, object_color_id, useSettingsStore.getState().show_structures);
  }
}

export function CameraInput() {
  const { gl, size, raycaster, camera } = useThree();

  /** Radians of pitch per pixel of vertical drag. Negative because dy grows
   * downward and dragging up is the natural "tilt toward the horizon". */
  function tilt_gain(): f64 {
    return -(MAX_POLAR - MIN_POLAR) / (TILT_SWEEP_FRACTION * size.height);
  }

  const drag_mode = useRef<DragMode>(DragMode.NONE);
  const did_drag = useRef(false);
  const pointer_x = useRef<f64>(0);
  const pointer_y = useRef<f64>(0);
  const rect_ref = useRef<DOMRect | null>(null);

  const last_x = useRef<f64>(0);
  const last_y = useRef<f64>(0);
  // pointerdown position for the current gesture — total displacement from
  // here (not the per-event dx/dy last_x/y tracks) is what decides whether
  // this is a tap or a drag. See TAP_DRAG_THRESHOLD_PX's doc comment.
  const tap_start_x = useRef<f64>(0);
  const tap_start_y = useRef<f64>(0);
  const $center = useRef<V3>([0, 0, 0]);
  const hover_raf_pending = useRef(false);

  // ---- drag-continuation coalescing ----
  // Raw pointermove fires far more often than the render loop (same reason
  // request_hover_update below exists) — do_pan/do_rotate/update_pinch all
  // write to zustand stores (set_map_center/set_view_state), and each write
  // fans out through reactive subscribers (chunk_stage.tsx's LOD system
  // among them). Profiled: without this, on_pointer_move dominated a whole
  // trace's CPU time during a drag. last_x/y and did_drag's tap-vs-drag
  // threshold still update on every raw event below (cheap ref writes, no
  // store write) — only the store-touching calls get deferred to once per
  // animation frame, coalescing however many raw events arrived since.
  const drag_raf_pending = useRef(false);
  const pending_pan_dx = useRef<f64>(0);
  const pending_pan_dy = useRef<f64>(0);
  const pending_rotate_dy = useRef<f64>(0);
  const pending_client_x = useRef<f64>(0);
  const pending_client_y = useRef<f64>(0);
  // wheel/trackpad-pinch zoom — same unthrottled-store-write shape as the
  // drag path above, and the more likely culprit for LOD-storm lag: zooming
  // changes camera distance, which can cross every visible chunk's LOD
  // threshold at once, unlike a pan (which mostly just changes *which*
  // chunks are visible). Summing deltaY before one exp() is exact — chained
  // multiplicative updates (exp(a)*exp(b) == exp(a+b)), not an approximation.
  const wheel_raf_pending = useRef(false);
  const pending_wheel_delta_y = useRef<f64>(0);

  // ---- rotate-drag (mouse right-drag / trackpad two-finger-click-drag) ----
  const rotate_start_angle = useRef<f64>(0);
  const rotate_start_azimuth = useRef<f64>(0);
  // whether rotate_start_angle/rotate_start_azimuth currently anchor a valid
  // reference sample (see MIN_ROTATE_RADIUS_PX doc comment in config.ts).
  const rotate_anchored = useRef(false);
  // pointer-down position for this rotate-drag — the reference point
  // AXIS_LOCK_THRESHOLD_PX measures total displacement against, separate
  // from last_x/last_y (which track the *previous move event*, not the
  // gesture's start) and from MIN_ROTATE_RADIUS_PX's screen-center-relative
  // radius (a different measurement for a different purpose — see
  // do_rotate()).
  const rotate_drag_start_x = useRef<f64>(0);
  const rotate_drag_start_y = useRef<f64>(0);
  const rotate_axis_lock = useRef<RotateAxisLock>(RotateAxisLock.NONE);

  // ---- multi-touch (pinch/twist/tilt) ----
  const active_pointers = useRef<Map<number, PointerPos>>(new Map());
  const pinch_start_dist = useRef<f64>(0);
  const pinch_start_angle = useRef<f64>(0);
  const pinch_start_midpoint_y = useRef<f64>(0);
  const pinch_start_view_dist = useRef<f64>(0);
  const pinch_start_azimuth = useRef<f64>(0);
  const pinch_start_polar = useRef<f64>(0);
  // whether pinch_start_angle/pinch_start_azimuth currently anchor a valid
  // twist reference sample (see MIN_TWIST_DIST_PX doc comment in config.ts).
  const twist_anchored = useRef(false);

  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_camera_status = useCameraStore.getState().set_camera_status;
  const set_hovered_coord = useCameraStore.getState().set_hovered_coord;
  const set_hovered_object_id = useCameraStore.getState().set_hovered_object_id;
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;

  const { world_frame_ref, local_world_frame_ref } = useFrameRefs();

  function update_rect() {
    rect_ref.current = gl.domElement.getBoundingClientRect();
  }

  function update_pointer(client_x: f64, client_y: f64) {
    let rect = rect_ref.current;
    if (!rect) {
      rect = gl.domElement.getBoundingClientRect();
      rect_ref.current = rect;
    }
    pointer_x.current = ((client_x - rect.left) / rect.width) * 2 - 1;
    pointer_y.current = -((client_y - rect.top) / rect.height) * 2 + 1;
  }

  /** Shared pan implementation — driven by either a mouse left-drag or a single touch. */
  function do_pan(dx: f64, dy: f64) {
    const view_state = useCameraStore.getState().view_state;
    const map_center: PVF3 | null = useMapStore.getState().map_center;
    if (!map_center) return;

    const view_h: f64 = 2 * view_state.dist * Math.tan((FOV * Math.PI) / 360);
    const units_per_px: f64 = view_h / size.height;

    const lx: f64 = -dx * units_per_px;
    const ly: f64 = dy * units_per_px;

    const azimuth: f64 = world_frame_ref.current?.rotation.z ?? view_state.azimuth;
    const wx: f64 = lx * Math.cos(azimuth) + ly * Math.sin(azimuth);
    const wy: f64 = -lx * Math.sin(azimuth) + ly * Math.cos(azimuth);

    const center: V3 = projection.unproject($center.current, map_center.i, map_center.f);
    const next_center: PVF3 = projection.project({ i: 0n, f: 0n }, center[0] + wx, center[1] + wy, center[2]);

    // ground_or_raw shares the exact same "zero Z, then sample real terrain
    // height" step every map_center-setting interaction that respects
    // ground_snap goes through (see its own doc comment) — the GPS
    // recenter button in tracking_button.tsx uses it too, instead of
    // each call site hand-rolling its own probe.
    const desired = useSettingsStore.getState().ground_snap ? ground_or_raw(next_center) : next_center;
    // Gated after the snap, not before: the snap moves the target's Z, and
    // the eye rides on it. Panning is the one control that moves the target,
    // and until it went through here the terrain gate could be walked
    // straight through sideways.
    set_map_center(allowed_pan(view_state, desired));
  }

  /** Azimuth tracks the cursor's angle around screen center since drag-start
   * (arcball-style), not a fixed px/degree rate — screen center is the fixed
   * pivot since the camera always looks at map_center. World rotates
   * opposite the cursor's angle delta (CCW world rotation reads as CW on
   * screen's flipped Y; verified live: +30° screen sweep -> azimuth -30°).
   * Exact for the cumulative rotational displacement since drag-start
   * (path-independent), but not pixel-perfect on a large net drag — a
   * single-axis arcball can only trace a fixed 1D arc on screen, which
   * won't generally pass through a far-away destination (an inherent limit,
   * not a formula bug).
   *
   * Polar (tilt) keeps the old fixed-rate drag — there's no natural anchor
   * point for tilt the way there is for a horizontal spin.
   *
   * The anchor is captured lazily, once the cursor first clears
   * MIN_ROTATE_RADIUS_PX from center — atan2 is degenerate right at the
   * pivot, so a drag starting in that dead zone re-anchors on the next
   * reliable sample instead of spinning.
   *
   * Axis lock (AXIS_LOCK_THRESHOLD_PX, see config.ts): azimuth and polar
   * used to update simultaneously every move event, but an off-center
   * "straight down" drag still sweeps some angle-around-center too — most
   * visible once polar saturated (MIN/MAX_POLAR, ground-collision clamp)
   * and the same drag suddenly read as a big, unrelated azimuth spin. Now
   * it commits to whichever axis dominates once past the threshold and
   * locks it for the rest of the gesture. */
  function do_rotate(client_x: f64, client_y: f64, dy: f64) {
    if (rotate_axis_lock.current === RotateAxisLock.NONE) {
      const total_dx: f64 = client_x - rotate_drag_start_x.current;
      const total_dy: f64 = client_y - rotate_drag_start_y.current;
      if (Math.hypot(total_dx, total_dy) < AXIS_LOCK_THRESHOLD_PX) return;
      rotate_axis_lock.current = Math.abs(total_dy) > Math.abs(total_dx) ? RotateAxisLock.TILT : RotateAxisLock.ROTATE;
    }

    const view_state = useCameraStore.getState().view_state;

    if (rotate_axis_lock.current === RotateAxisLock.TILT) {
      // lock_camera_tilt (settings_store.ts) only gates this write — the axis-
      // lock decision above still commits to TILT on a vertical drag either
      // way, it just becomes a no-op instead of changing polar. An
      // experiment toggle for a fixed-tilt UX, not yet a decided product
      // change, hence living in the debug menu rather than removing this
      // path outright.
      if (!useSettingsStore.getState().lock_camera_tilt) {
        set_view_state({ polar: view_state.polar + dy * tilt_gain() });
      }
      return;
    }

    const rect = rect_ref.current;
    let next_azimuth: f64 = view_state.azimuth;
    if (rect) {
      const cx: f64 = rect.left + rect.width / 2;
      const cy: f64 = rect.top + rect.height / 2;
      const dx: f64 = client_x - cx;
      const dyc: f64 = client_y - cy;
      const radius: f64 = Math.hypot(dx, dyc);

      // atan2's angle is numerically unstable close to the pivot — see
      // MIN_ROTATE_RADIUS_PX. Freeze azimuth and drop the anchor below that
      // radius; re-anchor from the next reliable sample once the cursor
      // moves back out, instead of tracking the unstable angle.
      if (radius >= MIN_ROTATE_RADIUS_PX) {
        const cur_angle: f64 = Math.atan2(dyc, dx);
        if (!rotate_anchored.current) {
          rotate_start_angle.current = cur_angle;
          rotate_start_azimuth.current = view_state.azimuth;
          rotate_anchored.current = true;
        } else {
          const angle_delta: f64 = cur_angle - rotate_start_angle.current;
          next_azimuth = rotate_start_azimuth.current - angle_delta;
        }
      } else {
        rotate_anchored.current = false;
      }
    }

    set_view_state({ azimuth: next_azimuth });
  }

  function pointer_pair(): [PointerPos, PointerPos] | null {
    if (active_pointers.current.size < 2) return null;
    const pts = [...active_pointers.current.values()];
    return [pts[0], pts[1]];
  }

  /** (Re)captures the two-finger baseline. Called on the 2nd finger touching down,
   * and again if a 3rd+ finger lifts back down to exactly 2, so the gesture never jumps. */
  function begin_pinch() {
    const pair = pointer_pair();
    if (!pair) return;
    const [p0, p1] = pair;

    pinch_start_dist.current = Math.hypot(p1.x - p0.x, p1.y - p0.y);
    pinch_start_midpoint_y.current = (p0.y + p1.y) / 2;
    // twist angle is re-anchored lazily by update_pinch() once the fingers
    // are far enough apart for that angle to be numerically reliable —
    // don't capture it here, it may be degenerate right at touch-down.
    twist_anchored.current = false;

    const view_state = useCameraStore.getState().view_state;
    pinch_start_view_dist.current = view_state.dist;
    pinch_start_azimuth.current = view_state.azimuth;
    pinch_start_polar.current = view_state.polar;

    drag_mode.current = DragMode.PINCH;
    did_drag.current = true;
    set_hovered_coord(null);
  }

  /** Pinch (spread/pinch -> zoom), twist (two-finger rotate -> azimuth), and
   * two-finger vertical drag (-> tilt/polar) — the standard mobile map gesture set. */
  function update_pinch() {
    const pair = pointer_pair();
    if (!pair || pinch_start_dist.current < 1) return;
    const [p0, p1] = pair;

    const dist: f64 = Math.hypot(p1.x - p0.x, p1.y - p0.y);
    const midpoint_y: f64 = (p0.y + p1.y) / 2;

    // fingers spreading apart (dist grows) zooms in (smaller camera distance).
    const scale: f64 = pinch_start_dist.current / dist;
    const next_dist: f64 = pinch_start_view_dist.current * scale;

    const midpoint_dy: f64 = midpoint_y - pinch_start_midpoint_y.current;
    const next_polar: f64 = pinch_start_polar.current + midpoint_dy * tilt_gain();

    // twist angle (inter-finger vector direction) is numerically unstable
    // when the fingers are close together — see MIN_TWIST_DIST_PX. Freeze
    // azimuth and drop the anchor below that distance; re-anchor from the
    // next reliable sample once the fingers spread back out, instead of
    // tracking the unstable angle.
    let next_azimuth: f64 = pinch_start_azimuth.current;
    if (dist >= MIN_TWIST_DIST_PX) {
      const angle: f64 = Math.atan2(p1.y - p0.y, p1.x - p0.x);
      if (!twist_anchored.current) {
        pinch_start_angle.current = angle;
        pinch_start_azimuth.current = useCameraStore.getState().view_state.azimuth;
        twist_anchored.current = true;
        next_azimuth = pinch_start_azimuth.current;
      } else {
        const angle_delta: f64 = angle - pinch_start_angle.current;
        next_azimuth = pinch_start_azimuth.current - angle_delta;
      }
    } else {
      twist_anchored.current = false;
    }

    // lock_camera_tilt (settings_store.ts) — same gate as do_rotate's TILT
    // branch, see its own comment. next_polar is still tracked/computed
    // above either way; this just decides whether it reaches the store.
    // The polar key must be omitted entirely when locked, not set to
    // undefined — set_view_state spreads its argument over the existing
    // view_state, so an explicit `polar: undefined` would overwrite the
    // real value with undefined instead of leaving it untouched.
    const view_update: { dist: f64; azimuth: f64; polar?: f64 } = { dist: next_dist, azimuth: next_azimuth };
    if (!useSettingsStore.getState().lock_camera_tilt) view_update.polar = next_polar;
    set_view_state(view_update);
  }

  function on_pointer_down(e: PointerEvent) {
    if (e.pointerType === "touch") {
      active_pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
      update_rect();
      set_camera_status(CameraStatus.DRAG);
      set_is_follow_mode(false);

      if (active_pointers.current.size === 1) {
        drag_mode.current = DragMode.PAN;
        did_drag.current = false;
        last_x.current = e.clientX;
        last_y.current = e.clientY;
        tap_start_x.current = e.clientX;
        tap_start_y.current = e.clientY;
        update_pointer(e.clientX, e.clientY);
      } else if (active_pointers.current.size === 2) {
        begin_pinch();
      }
      // 3rd+ finger: ignore, keep the current 2-finger gesture as-is.
      return;
    }

    if (e.button === 0) drag_mode.current = DragMode.PAN;
    else if (e.button === 2) drag_mode.current = DragMode.ROTATE;
    else return;

    update_rect();
    did_drag.current = false;

    last_x.current = e.clientX;
    last_y.current = e.clientY;
    tap_start_x.current = e.clientX;
    tap_start_y.current = e.clientY;

    if (drag_mode.current === DragMode.ROTATE) {
      // anchor is captured lazily by do_rotate() on the first reliable
      // sample instead of here — the down position itself may be within
      // MIN_ROTATE_RADIUS_PX of the pivot, where atan2's angle is degenerate.
      rotate_anchored.current = false;
      rotate_axis_lock.current = RotateAxisLock.NONE;
      rotate_drag_start_x.current = e.clientX;
      rotate_drag_start_y.current = e.clientY;
    }

    update_pointer(e.clientX, e.clientY);
    set_camera_status(CameraStatus.DRAG);
    set_is_follow_mode(false);
  }

  function request_hover_update() {
    if (hover_raf_pending.current) return;
    hover_raf_pending.current = true;

    requestAnimationFrame(() => {
      hover_raf_pending.current = false;
      // pointer may have started dragging (or left) since this was scheduled.
      if (drag_mode.current !== DragMode.NONE) return;
      update_hover();
    });
  }

  function request_drag_flush() {
    if (drag_raf_pending.current) return;
    drag_raf_pending.current = true;

    requestAnimationFrame(() => {
      drag_raf_pending.current = false;
      // gesture may have ended (pointerup) since this was scheduled.
      if (drag_mode.current === DragMode.PAN) {
        do_pan(pending_pan_dx.current, pending_pan_dy.current);
      } else if (drag_mode.current === DragMode.ROTATE) {
        do_rotate(pending_client_x.current, pending_client_y.current, pending_rotate_dy.current);
      } else if (drag_mode.current === DragMode.PINCH) {
        update_pinch();
      }
      pending_pan_dx.current = 0;
      pending_pan_dy.current = 0;
      pending_rotate_dy.current = 0;
    });
  }

  function on_pointer_move(e: PointerEvent) {
    if (e.pointerType === "touch") {
      const pt = active_pointers.current.get(e.pointerId);
      if (pt) {
        pt.x = e.clientX;
        pt.y = e.clientY;
      }

      if (drag_mode.current === DragMode.PINCH && active_pointers.current.size >= 2) {
        request_drag_flush();
        return;
      }

      if (drag_mode.current === DragMode.PAN && active_pointers.current.size === 1) {
        const dx: f64 = e.clientX - last_x.current;
        const dy: f64 = e.clientY - last_y.current;
        if (Math.hypot(e.clientX - tap_start_x.current, e.clientY - tap_start_y.current) > TAP_DRAG_THRESHOLD_PX) {
          did_drag.current = true;
        }
        last_x.current = e.clientX;
        last_y.current = e.clientY;
        pending_pan_dx.current += dx;
        pending_pan_dy.current += dy;
        request_drag_flush();
      }
      return;
    }

    update_pointer(e.clientX, e.clientY);

    if (drag_mode.current === DragMode.NONE) {
      // raycasting against terrain is expensive (up to ~200 marching steps); a raw
      // pointermove can fire far more often than the render loop, so cap it to once
      // per animation frame instead of once per event.
      request_hover_update();
      return;
    }

    const dx: f64 = e.clientX - last_x.current;
    const dy: f64 = e.clientY - last_y.current;
    if (Math.hypot(e.clientX - tap_start_x.current, e.clientY - tap_start_y.current) > TAP_DRAG_THRESHOLD_PX) {
      did_drag.current = true;
    }

    last_x.current = e.clientX;
    last_y.current = e.clientY;
    pending_client_x.current = e.clientX;
    pending_client_y.current = e.clientY;

    if (drag_mode.current === DragMode.ROTATE) {
      pending_rotate_dy.current += dy;
      request_drag_flush();
      return;
    }

    pending_pan_dx.current += dx;
    pending_pan_dy.current += dy;
    request_drag_flush();
  }

  function clear_hovered_once() {
    const state = useCameraStore.getState();
    if (state.hovered_coord !== null) set_hovered_coord(null);
    if (state.hovered_object_id !== null) set_hovered_object_id(null);
  }

  function update_hover() {
    const map_center = useMapStore.getState().map_center;
    if (!map_center) {
      clear_hovered_once();
      return;
    }

    $pointer.set(pointer_x.current, pointer_y.current);
    raycaster.setFromCamera($pointer, camera);

    const { activeTool, object_size } = useUiStore.getState();
    const map_origin = get_local_world_origin(map_center);

    // One unified raycast — terrain (top or a wall) and any placed object
    // (any size) all come back as the same WorldHit shape now, since
    // they're all just boxes in the same dense voxel field (ray.ts's own
    // doc comment). No more per-tool "do we need the wall pass" gating —
    // it's not a separate pass anymore.
    const pick = ray_pick($ray, raycaster.ray, world_frame_ref.current, local_world_frame_ref.current, map_center);
    if (!pick) {
      clear_hovered_once();
      return;
    }
    const { hit } = pick;

    if (activeTool === "object" || activeTool === "decor") {
      // the one tool group that wants an ADJACENT empty slot, not the
      // struck surface itself. Decor always places at its own fixed
      // DECOR_SIZE, never the object tool's own adjustable object_size.
      set_hovered_object_id(null);
      const placement_size = activeTool === "decor" ? DECOR_SIZE : object_size;

      // When the struck thing is a real, known OccupiedEntry, hand
      // world_adjacent_corner its exact stored AABB instead of letting it
      // fall back to hit's own box_min/box_max — those come back through
      // the u8-packed sub_cell payload (voxel.ts), accurate to only
      // ~1/510 of a cell, which is what let stacked sub-cell placements
      // silently drift (see world_adjacent_corner's own doc comment on
      // precise_box). Terrain/no-resolved-object hits have no more precise
      // source, so this is null and world_adjacent_corner uses hit's own
      // box as before. resolve_hit_target also resolves a real mesh hit
      // against baked decor (decor_hit.ts) — see that function's own doc
      // comment.
      const struck_object = resolve_hit_target(raycaster, hit, map_origin);
      const precise_box = struck_object ? object_aabb(struck_object, map_origin) : undefined;

      set_hovered_coord(world_adjacent_corner(hit, placement_size, map_origin, precise_box));
      return;
    }

    // pointer (recenter on it) and eraser (remove/dig it) both target the
    // struck thing itself, not an adjacent slot.
    const struck_object = resolve_hit_target(raycaster, hit, map_origin);
    set_hovered_object_id(struck_object?.id ?? null);
    if (struck_object) {
      set_hovered_coord({ i: struck_object.i, f: struck_object.f });
    } else if (activeTool === "eraser" && useSettingsStore.getState().enable_terrain_carving) {
      // Bare terrain under eraser wants the struck voxel's OWN corner (real,
      // possibly already-dug-into height included), not world_hover_pvf3's
      // baked-elevation "flush on top" slot below — see world_struck_corner's
      // own doc comment for why those two disagree and eraser needs the
      // former to actually reach the terrain itself. Gated on
      // enable_terrain_carving (settings/slices/camera_slice.ts) the same as
      // handle_tap's own write — with it off, eraser falls through to the
      // exact same bare-terrain branch pointer already uses, just below.
      set_hovered_coord(world_struck_corner(hit, object_size, map_origin));
    } else {
      // pointer, and eraser with terrain-carving off — the struck cell's own
      // owning corner, grounded to its real top (for recentering the
      // camera, not editing).
      const grounded = world_hover_pvf3(hit, map_origin);
      grounded ? set_hovered_coord(grounded) : clear_hovered_once();
    }
  }

  function on_pointer_up(e: PointerEvent) {
    if (e.pointerType === "touch") {
      active_pointers.current.delete(e.pointerId);

      if (active_pointers.current.size === 0) {
        const mode = drag_mode.current;
        const was_active = mode !== DragMode.NONE;
        drag_mode.current = DragMode.NONE;
        set_camera_status(CameraStatus.IDLE);

        if (was_active && !did_drag.current && mode === DragMode.PAN) {
          update_hover();
          const hovered = useCameraStore.getState().hovered_coord;
          if (hovered) handle_tap(hovered);
        }
      } else if (active_pointers.current.size === 1) {
        // dropped from 2 fingers to 1 — resume single-finger pan seamlessly.
        const [remaining] = [...active_pointers.current.values()];
        drag_mode.current = DragMode.PAN;
        last_x.current = remaining.x;
        last_y.current = remaining.y;
        did_drag.current = true; // already mid-gesture, don't treat the eventual lift as a tap
      } else {
        // dropped from 3+ fingers to exactly 2 — recalibrate the pinch baseline.
        begin_pinch();
      }
      return;
    }

    if (drag_mode.current === DragMode.NONE) return;

    const mode = drag_mode.current;
    drag_mode.current = DragMode.NONE;
    set_camera_status(CameraStatus.IDLE);

    if (!did_drag.current && mode === DragMode.PAN) {
      update_hover();
      const hovered = useCameraStore.getState().hovered_coord;
      if (hovered) handle_tap(hovered);
    }
  }

  function on_pointer_out() {
    clear_hovered_once();
  }

  function request_wheel_flush() {
    if (wheel_raf_pending.current) return;
    wheel_raf_pending.current = true;

    requestAnimationFrame(() => {
      wheel_raf_pending.current = false;
      const view_state = useCameraStore.getState().view_state;
      set_view_state({ dist: view_state.dist * Math.exp(pending_wheel_delta_y.current * DOLLY_GAIN) });
      pending_wheel_delta_y.current = 0;
    });
  }

  function on_wheel(e: WheelEvent) {
    if (e.ctrlKey) e.preventDefault();

    pending_wheel_delta_y.current += e.deltaY;
    request_wheel_flush();
  }

  useEffect(() => {
    const el = gl.domElement;

    // prevent the browser's native touch scroll/pinch-zoom from fighting with our
    // own pan/pinch handling.
    el.style.touchAction = "none";

    function on_context_menu(e: MouseEvent) {
      e.preventDefault();
    }

    function on_resize() {
      update_rect();
    }

    function on_pointer_leave() {
      on_pointer_out();
    }

    update_rect();

    el.addEventListener("pointerdown", on_pointer_down);
    el.addEventListener("pointermove", on_pointer_move);
    el.addEventListener("pointerleave", on_pointer_leave);
    el.addEventListener("contextmenu", on_context_menu);
    el.addEventListener("wheel", on_wheel, { passive: false });
    window.addEventListener("resize", on_resize);
    window.addEventListener("scroll", on_resize, true);
    window.addEventListener("pointerup", on_pointer_up);
    window.addEventListener("pointercancel", on_pointer_up);
    return () => {
      el.removeEventListener("pointerdown", on_pointer_down);
      el.removeEventListener("pointermove", on_pointer_move);
      el.removeEventListener("pointerleave", on_pointer_leave);
      el.removeEventListener("contextmenu", on_context_menu);
      el.removeEventListener("wheel", on_wheel);
      window.removeEventListener("resize", on_resize);
      window.removeEventListener("scroll", on_resize, true);
      window.removeEventListener("pointerup", on_pointer_up);
      window.removeEventListener("pointercancel", on_pointer_up);
    };
  }, [gl]);

  return null;
}
