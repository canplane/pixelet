// @pxl/web/modules/camera/input.tsx

import { useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useFrameRefs } from "@/modules/world/frame/context.tsx";
import { ray_pick_terrain } from "./ray.ts";
import { CameraStatus, useCameraStore } from "./store.ts";
import { camera_cfg } from "./config.ts";

const { FOV } = camera_cfg.projection;
const { ROT_GAIN_AZIMUTH, ROT_GAIN_POLAR, DOLLY_GAIN } = camera_cfg.input;

const enum DragMode {
  NONE = "none",
  PAN = "pan",
  ROTATE = "rotate",
}

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

  function on_pointer_down(e: PointerEvent) {
    if (e.button === 0) drag_mode.current = DragMode.PAN;
    else if (e.button === 2) drag_mode.current = DragMode.ROTATE;
    else return;

    update_rect();
    did_drag.current = false;

    last_x.current = e.clientX;
    last_y.current = e.clientY;

    update_pointer(e.clientX, e.clientY);
    set_camera_status(CameraStatus.DRAG);
    set_is_follow_mode(false);
    set_hovered_coord(null);
  }

  function on_pointer_move(e: PointerEvent) {
    update_pointer(e.clientX, e.clientY);

    if (drag_mode.current === DragMode.NONE) {
      update_hover();
      return;
    }

    const dx: f64 = e.clientX - last_x.current;
    const dy: f64 = e.clientY - last_y.current;
    if (dx !== 0 || dy !== 0) did_drag.current = true;

    const prev_x = last_x.current;
    const prev_y = last_y.current;

    last_x.current = e.clientX;
    last_y.current = e.clientY;

    if (drag_mode.current === DragMode.ROTATE) {
      const view_state = useCameraStore.getState().view_state;

      const cx: f64 = size.width * 0.5;
      const cy: f64 = size.height * 0.5;

      const mx: f64 = (prev_x + e.clientX) * 0.5 - cx;
      const my: f64 = cy - (prev_y + e.clientY) * 0.5;

      const r: f64 = Math.max(1, Math.hypot(mx, my));

      // screen-center-relative tangential drag controls azimuth
      const tangent: f64 = (mx * dy + my * dx) / r;

      set_view_state({
        azimuth: view_state.azimuth + tangent * ROT_GAIN_AZIMUTH,
        polar: view_state.polar + dy * ROT_GAIN_POLAR,
      });
      return;
    }

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
    set_map_center(next_center);
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

  function on_pointer_up(_e: PointerEvent) {
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
