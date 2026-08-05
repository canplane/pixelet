// @pxl/web/modules/camera/input.tsx

import { useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";

import { projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useFrameRefs } from "@/modules/world/frame/context.tsx";
import { with_ele } from "@/modules/world/terrain/use-elevation.ts";
import { ray_pick_terrain } from "./ray.ts";
import { CameraStatus, useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";

const { FOV } = camera_cfg.projection;
const { ROT_GAIN_POLAR, ROT_GAIN_REF_HEIGHT, DOLLY_GAIN } = camera_cfg.input;

const enum DragMode {
  NONE = "none",
  PAN = "pan",
  ROTATE = "rotate",
  PINCH = "pinch",
}

type PointerPos = { x: f64; y: f64 };

const $ray = new THREE.Ray();
const $pointer = new THREE.Vector2();

export function CameraInput() {
  const { gl, size, raycaster, camera } = useThree();

  const drag_mode = useRef<DragMode>(DragMode.NONE);
  const did_drag = useRef(false);
  const pointer_x = useRef<f64>(0);
  const pointer_y = useRef<f64>(0);
  const rect_ref = useRef<DOMRect | null>(null);

  const last_x = useRef<f64>(0);
  const last_y = useRef<f64>(0);
  const $center = useRef<V3>([0, 0, 0]);
  const hover_raf_pending = useRef(false);

  // ---- rotate-drag (mouse right-drag / trackpad two-finger-click-drag) ----
  const rotate_start_angle = useRef<f64>(0);
  const rotate_start_azimuth = useRef<f64>(0);

  // ---- multi-touch (pinch/twist/tilt) ----
  const active_pointers = useRef<Map<number, PointerPos>>(new Map());
  const pinch_start_dist = useRef<f64>(0);
  const pinch_start_angle = useRef<f64>(0);
  const pinch_start_midpoint_y = useRef<f64>(0);
  const pinch_start_view_dist = useRef<f64>(0);
  const pinch_start_azimuth = useRef<f64>(0);
  const pinch_start_polar = useRef<f64>(0);

  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_camera_status = useCameraStore.getState().set_camera_status;
  const set_hovered_coord = useCameraStore.getState().set_hovered_coord;
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

    if (useCameraStore.getState().follow_terrain) {
      // with_ele() adds sampled elevation onto out's existing Z, so the probe's Z must start at 0
      // (matches ray.ts's ray_hit_terrain convention) — otherwise height compounds every drag step.
      const ground_probe: PVF3 = projection.project({ i: 0n, f: 0n }, center[0] + wx, center[1] + wy, 0);
      const grounded: PVF3 | null = with_ele(ground_probe, ground_probe.i, ground_probe.f);
      set_map_center(grounded ?? next_center);
    } else {
      set_map_center(next_center);
    }
  }

  /** Azimuth for a rotate-drag: matches the cursor's angle-around-screen-center delta
   * since drag-start (arcball-style), instead of a fixed px/degree rate. The camera
   * always looks straight at map_center, so screen center is the one fixed pivot to
   * measure against. World rotates opposite the cursor's angle delta because
   * increasing azimuth spins world points CCW (viewed from above) while screen space
   * is Y-flipped, so a world point's *screen* angle moves by -azimuth_delta (verified
   * live: a synthetic two-finger sweep of +30° screen angle mapped to azimuth -30°,
   * matching this sign).
   *
   * This matches the cursor's rotational *rate* exactly (correct for the total
   * displacement accumulated since drag-start, not just per-frame deltas — the formula
   * is path-independent), so it's accurate while that cumulative displacement is small.
   * It is not literal pixel-perfect tracking for a large net drag: with azimuth as the
   * only degree of freedom, a grabbed point only ever moves along a fixed 1D arc on
   * screen, which generally does not pass through an arbitrary far-away destination
   * (checked numerically against the real perspective+tilt projection — moderate drags
   * land within a few px, large swings can be tens of px off). This is an inherent
   * limit of any single-axis arcball drag, not something a better formula fixes.
   *
   * Polar (tilt) keeps the old fixed-rate drag — there's no natural "grabbed point"
   * to anchor for tilt the way there is for a horizontal spin. */
  function do_rotate(client_x: f64, client_y: f64, dy: f64) {
    const rect = rect_ref.current;
    const view_state = useCameraStore.getState().view_state;
    const norm: f64 = ROT_GAIN_REF_HEIGHT / size.height;

    let next_azimuth: f64 = view_state.azimuth;
    if (rect) {
      const cx: f64 = rect.left + rect.width / 2;
      const cy: f64 = rect.top + rect.height / 2;
      const cur_angle: f64 = Math.atan2(client_y - cy, client_x - cx);
      const angle_delta: f64 = cur_angle - rotate_start_angle.current;
      next_azimuth = rotate_start_azimuth.current - angle_delta;
    }

    set_view_state({
      azimuth: next_azimuth,
      polar: view_state.polar + dy * ROT_GAIN_POLAR * norm,
    });
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
    pinch_start_angle.current = Math.atan2(p1.y - p0.y, p1.x - p0.x);
    pinch_start_midpoint_y.current = (p0.y + p1.y) / 2;

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
    const angle: f64 = Math.atan2(p1.y - p0.y, p1.x - p0.x);
    const midpoint_y: f64 = (p0.y + p1.y) / 2;

    // Fingers spreading apart (dist grows) zooms in (smaller camera distance).
    const scale: f64 = pinch_start_dist.current / dist;
    const next_dist: f64 = pinch_start_view_dist.current * scale;

    const angle_delta: f64 = angle - pinch_start_angle.current;
    const next_azimuth: f64 = pinch_start_azimuth.current - angle_delta;

    const norm: f64 = ROT_GAIN_REF_HEIGHT / size.height;
    const midpoint_dy: f64 = midpoint_y - pinch_start_midpoint_y.current;
    const next_polar: f64 = pinch_start_polar.current + midpoint_dy * ROT_GAIN_POLAR * norm;

    set_view_state({ dist: next_dist, azimuth: next_azimuth, polar: next_polar });
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
        update_pointer(e.clientX, e.clientY);
        set_hovered_coord(null);
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

    if (drag_mode.current === DragMode.ROTATE) {
      const rect = rect_ref.current;
      if (rect) {
        const cx: f64 = rect.left + rect.width / 2;
        const cy: f64 = rect.top + rect.height / 2;
        rotate_start_angle.current = Math.atan2(e.clientY - cy, e.clientX - cx);
      }
      rotate_start_azimuth.current = useCameraStore.getState().view_state.azimuth;
    }

    update_pointer(e.clientX, e.clientY);
    set_camera_status(CameraStatus.DRAG);
    set_is_follow_mode(false);
    set_hovered_coord(null);
  }

  function request_hover_update() {
    if (hover_raf_pending.current) return;
    hover_raf_pending.current = true;

    requestAnimationFrame(() => {
      hover_raf_pending.current = false;
      // Pointer may have started dragging (or left) since this was scheduled.
      if (drag_mode.current !== DragMode.NONE) return;
      update_hover();
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
        update_pinch();
        return;
      }

      if (drag_mode.current === DragMode.PAN && active_pointers.current.size === 1) {
        const dx: f64 = e.clientX - last_x.current;
        const dy: f64 = e.clientY - last_y.current;
        if (dx !== 0 || dy !== 0) did_drag.current = true;
        last_x.current = e.clientX;
        last_y.current = e.clientY;
        do_pan(dx, dy);
      }
      return;
    }

    update_pointer(e.clientX, e.clientY);

    if (drag_mode.current === DragMode.NONE) {
      // Raycasting against terrain is expensive (up to ~200 marching steps); a raw
      // pointermove can fire far more often than the render loop, so cap it to once
      // per animation frame instead of once per event.
      request_hover_update();
      return;
    }

    const dx: f64 = e.clientX - last_x.current;
    const dy: f64 = e.clientY - last_y.current;
    if (dx !== 0 || dy !== 0) did_drag.current = true;

    last_x.current = e.clientX;
    last_y.current = e.clientY;

    if (drag_mode.current === DragMode.ROTATE) {
      do_rotate(e.clientX, e.clientY, dy);
      return;
    }

    do_pan(dx, dy);
  }

  function clear_hovered_once() {
    if (useCameraStore.getState().hovered_coord !== null) {
      set_hovered_coord(null);
    }
  }

  function update_hover() {
    const map_center = useMapStore.getState().map_center;
    if (!map_center) {
      clear_hovered_once();
      return;
    }

    $pointer.set(pointer_x.current, pointer_y.current);
    raycaster.setFromCamera($pointer, camera);

    const hit: PVF3 | null = ray_pick_terrain($ray, raycaster.ray, world_frame_ref.current, local_world_frame_ref.current, map_center);
    if (!hit) {
      clear_hovered_once();
      return;
    }

    set_hovered_coord(hit);
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
          if (hovered) set_map_center(hovered);
        }
      } else if (active_pointers.current.size === 1) {
        // Dropped from 2 fingers to 1 — resume single-finger pan seamlessly.
        const [remaining] = [...active_pointers.current.values()];
        drag_mode.current = DragMode.PAN;
        last_x.current = remaining.x;
        last_y.current = remaining.y;
        did_drag.current = true; // already mid-gesture, don't treat the eventual lift as a tap
      } else {
        // Dropped from 3+ fingers to exactly 2 — recalibrate the pinch baseline.
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
      if (hovered) {
        set_map_center(hovered);
      }
    }
  }

  function on_pointer_out() {
    clear_hovered_once();
  }

  function on_wheel(e: WheelEvent) {
    if (e.ctrlKey) e.preventDefault();

    const view_state = useCameraStore.getState().view_state;
    set_view_state({ dist: view_state.dist * Math.exp(e.deltaY * DOLLY_GAIN) });
  }

  useEffect(() => {
    const el = gl.domElement;

    // Prevent the browser's native touch scroll/pinch-zoom from fighting with our
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
