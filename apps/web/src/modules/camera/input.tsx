// @pxl/web/modules/camera/input.tsx

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { get_render_camera_state, get_render_world_state } from "@/render/runtime.ts";
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

type InstallCameraInputProps = {
  canvas: HTMLCanvasElement;
  get_world_azimuth: () => f64;
};

export function install_camera_input(props: InstallCameraInputProps): () => void {
  const { canvas, get_world_azimuth } = props;

  let drag_mode: DragMode = DragMode.NONE;
  let did_drag = false;

  let pointer_x: f64 = 0;
  let pointer_y: f64 = 0;

  let last_x: f64 = 0;
  let last_y: f64 = 0;

  let rect: DOMRect | null = null;
  const center: V3 = [0, 0, 0];

  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_camera_status = useCameraStore.getState().set_camera_status;
  const set_hovered_coord = useCameraStore.getState().set_hovered_coord;
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;

  function update_rect() {
    rect = canvas.getBoundingClientRect();
  }

  function update_pointer(client_x: f64, client_y: f64) {
    let next_rect = rect;
    if (!next_rect) {
      next_rect = canvas.getBoundingClientRect();
      rect = next_rect;
    }

    pointer_x = ((client_x - next_rect.left) / next_rect.width) * 2 - 1;
    pointer_y = -((client_y - next_rect.top) / next_rect.height) * 2 + 1;
  }

  function clear_hovered_once() {
    if (useCameraStore.getState().hovered_coord !== null) {
      set_hovered_coord(null);
    }
  }

  function update_hover() {
    const map_center: PVF3 | null = useMapStore.getState().map_center;
    if (!map_center) {
      clear_hovered_once();
      return;
    }

    const camera = get_render_camera_state();
    const world = get_render_world_state();
    if (!camera || !world) {
      clear_hovered_once();
      return;
    }

    const hit: PVF3 | null = ray_pick_terrain(
      [pointer_x, pointer_y],
      camera,
      world,
      map_center,
    );

    if (!hit) {
      clear_hovered_once();
      return;
    }

    set_hovered_coord(hit);
  }

  function on_pointer_down(e: PointerEvent) {
    if (e.button === 0) drag_mode = DragMode.PAN;
    else if (e.button === 2) drag_mode = DragMode.ROTATE;
    else return;

    update_rect();
    did_drag = false;

    last_x = e.clientX;
    last_y = e.clientY;

    update_pointer(e.clientX, e.clientY);
    set_camera_status(CameraStatus.DRAG);
    set_is_follow_mode(false);
    set_hovered_coord(null);
  }

  function on_pointer_move(e: PointerEvent) {
    update_pointer(e.clientX, e.clientY);

    if (drag_mode === DragMode.NONE) {
      update_hover();
      return;
    }

    const dx: f64 = e.clientX - last_x;
    const dy: f64 = e.clientY - last_y;
    if (dx !== 0 || dy !== 0) did_drag = true;

    const prev_x = last_x;
    const prev_y = last_y;

    last_x = e.clientX;
    last_y = e.clientY;

    if (drag_mode === DragMode.ROTATE) {
      const view_state = useCameraStore.getState().view_state;

      const cx: f64 = canvas.width * 0.5;
      const cy: f64 = canvas.height * 0.5;

      const mx: f64 = (prev_x + e.clientX) * 0.5 - cx;
      const my: f64 = cy - (prev_y + e.clientY) * 0.5;

      const r: f64 = Math.max(1, Math.hypot(mx, my));
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
    const units_per_px: f64 = view_h / Math.max(1, canvas.height);

    const lx: f64 = -dx * units_per_px;
    const ly: f64 = dy * units_per_px;

    const azimuth: f64 = get_world_azimuth();
    const wx: f64 = lx * Math.cos(azimuth) + ly * Math.sin(azimuth);
    const wy: f64 = -lx * Math.sin(azimuth) + ly * Math.cos(azimuth);

    const c: V3 = projection.unproject(center, map_center.i, map_center.f);
    const next_center: PVF3 = projection.project({ i: 0n, f: 0n }, c[0] + wx, c[1] + wy, c[2]);
    set_map_center(next_center);
  }

  function on_pointer_up(_e: PointerEvent) {
    if (drag_mode === DragMode.NONE) return;

    const mode = drag_mode;
    drag_mode = DragMode.NONE;
    set_camera_status(CameraStatus.IDLE);

    if (!did_drag && mode === DragMode.PAN) {
      update_hover();
      const hovered = useCameraStore.getState().hovered_coord;
      if (hovered) {
        set_map_center(hovered);
        return;
      }
    }

    clear_hovered_once();
  }

  function on_pointer_leave() {
    clear_hovered_once();
  }

  function on_wheel(e: WheelEvent) {
    if (e.ctrlKey) e.preventDefault();

    const view_state = useCameraStore.getState().view_state;
    set_view_state({ dist: view_state.dist * Math.exp(e.deltaY * DOLLY_GAIN) });
  }

  function on_context_menu(e: MouseEvent) {
    e.preventDefault();
  }

  function on_resize() {
    update_rect();
  }

  update_rect();

  canvas.addEventListener("pointerdown", on_pointer_down);
  canvas.addEventListener("pointermove", on_pointer_move);
  canvas.addEventListener("pointerleave", on_pointer_leave);
  canvas.addEventListener("contextmenu", on_context_menu);
  canvas.addEventListener("wheel", on_wheel, { passive: false });

  window.addEventListener("resize", on_resize);
  window.addEventListener("scroll", on_resize, true);
  window.addEventListener("pointerup", on_pointer_up);
  window.addEventListener("pointercancel", on_pointer_up);

  return () => {
    canvas.removeEventListener("pointerdown", on_pointer_down);
    canvas.removeEventListener("pointermove", on_pointer_move);
    canvas.removeEventListener("pointerleave", on_pointer_leave);
    canvas.removeEventListener("contextmenu", on_context_menu);
    canvas.removeEventListener("wheel", on_wheel);

    window.removeEventListener("resize", on_resize);
    window.removeEventListener("scroll", on_resize, true);
    window.removeEventListener("pointerup", on_pointer_up);
    window.removeEventListener("pointercancel", on_pointer_up);
  };
}
