// @pxl/web/modules/camera/store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";
import { round_to_step } from "@xpute/spec/lib/kit/math.ts";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";

import type { ViewState } from "@/modules/camera/lib/pose.ts";
import { camera_cfg } from "./config.ts";

const { DEFAULT_POLAR_ANGLE, DEFAULT_DIST } = camera_cfg;

export const enum CameraStatus {
  IDLE = "idle",
  DRAG = "drag",
  FLYING = "flying",
}

export type CameraStore = {
  camera_status: CameraStatus;
  set_camera_status: (camera_status: CameraStatus) => void;

  hovered_coord: PVF3 | null;
  set_hovered_coord: (coord: PVF3 | null) => void;
  clear_hovered_coord: () => void;

  /** User/system-facing camera view intent. */
  view_state: ViewState;
  set_view_state: (partial: Partial<ViewState>) => void;

  // ---- Control Signals ----

  /** Reset signals for camera controllers. */
  camera_reset_signal: { dist: f64; orientation: f64 };
  trigger_dist_reset: () => void;
  trigger_orientation_reset: () => void;

  // ---- Interaction ----

  /** Follow mode flag. */
  is_follow_mode: boolean;
  set_is_follow_mode: (is_follow_mode: boolean) => void;
};

export const useCameraStore = create<CameraStore>((set) => ({
  camera_status: CameraStatus.IDLE,
  set_camera_status: (camera_status) => set({ camera_status }),

  hovered_coord: null,
  set_hovered_coord: (hovered_coord) => set({ hovered_coord }),
  clear_hovered_coord: () => set({ hovered_coord: null }),

  view_state: {
    dist: DEFAULT_DIST,
    bearing: 0,
    polar_angle: DEFAULT_POLAR_ANGLE,
    height: 100,
  },
  set_view_state: (partial) =>
    set((state) => {
      const next = { ...state.view_state, ...partial };

      next.dist = round_to_step(next.dist, .1);
      next.height = round_to_step(next.height, .1);
      next.bearing = round_to_step(next.bearing, .1);
      next.polar_angle = round_to_step(next.polar_angle, .01);

      return { view_state: next };
    }),

  camera_reset_signal: { dist: 0, orientation: 0 },
  trigger_dist_reset: () =>
    set((state) => ({
      camera_reset_signal: { ...state.camera_reset_signal, dist: Date.now() },
    })),
  trigger_orientation_reset: () =>
    set((state) => ({
      camera_reset_signal: { ...state.camera_reset_signal, orientation: Date.now() },
    })),

  is_follow_mode: false,
  set_is_follow_mode: (is_follow_mode) => set({ is_follow_mode }),
}));
