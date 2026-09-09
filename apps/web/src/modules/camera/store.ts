// @pxl/web/modules/camera/store.ts

import { allowed_step } from "./terrain_gate.ts";
import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp, round_to_step, wrap } from "@xpute/core/kit/math.ts";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { camera_cfg } from "./config.ts";

const { DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MIN_DIST, MAX_DIST, MIN_POLAR, MAX_POLAR } = camera_cfg.view;

const norm_dist = (dist: f64): f64 => clamp(round_to_step(dist, .1), MIN_DIST, MAX_DIST);
const norm_azimuth = (azimuth: f64): f64 => wrap(round_to_step(azimuth, 0.1 * (Math.PI / 180)), -Math.PI, Math.PI);
const norm_polar = (polar: f64): f64 => clamp(round_to_step(polar, .01), MIN_POLAR, MAX_POLAR);

export const enum CameraStatus {
  IDLE = "idle",
  DRAG = "drag",
}

/**
 * Camera view intent.
 *
 * This is not the raw physical camera transform.
 * It stores the user/system-facing view parameters used to derive a pose.
 */
export type ViewState = {
  /** Camera-to-target distance. */
  dist: f64;
  /** Orbit azimuth in radians, normalized to [-PI, PI]. */
  azimuth: f64;
  /** Orbit polar angle in radians. */
  polar: f64;
};

export type CameraStore = {
  camera_status: CameraStatus;
  set_camera_status: (camera_status: CameraStatus) => void;

  hovered_coord: PVF3 | null;
  set_hovered_coord: (coord: PVF3 | null) => void;

  /** id of the placed object currently under the cursor, if any — see camera/ray.ts's PickResult.hit_object. */
  hovered_object_id: string | null;
  set_hovered_object_id: (id: string | null) => void;

  /** User/system-facing camera view intent. */
  view_state: ViewState;
  set_view_state: (partial: Partial<ViewState>) => void;

  // ---- Control Signals ----

  /** Reset signals for camera controllers. */
  reset_dist: () => void;
  reset_azimuth: () => void;
  reset_view: () => void;

  // ---- Interaction ----

  /** Follow mode flag. */
  is_follow_mode: boolean;
  set_is_follow_mode: (is_follow_mode: boolean) => void;
};

export const useCameraStore = create<CameraStore>((set) => ({
  camera_status: CameraStatus.IDLE,
  set_camera_status: (camera_status) => set((state) => state.camera_status === camera_status ? state : { camera_status }),

  hovered_coord: null,
  set_hovered_coord: (hovered_coord) =>
    set((state) => {
      const same: boolean = state.hovered_coord?.i === hovered_coord?.i && state.hovered_coord?.f === hovered_coord?.f;
      return same ? state : { hovered_coord };
    }),

  hovered_object_id: null,
  set_hovered_object_id: (hovered_object_id) => set((state) => state.hovered_object_id === hovered_object_id ? state : { hovered_object_id }),

  view_state: {
    dist: DEFAULT_DIST,
    azimuth: DEFAULT_AZIMUTH,
    polar: DEFAULT_POLAR,
  },
  set_view_state: (partial) =>
    set((state) => {
      const next = { ...state.view_state, ...partial };

      next.dist = norm_dist(next.dist);
      next.azimuth = norm_azimuth(next.azimuth);
      next.polar = norm_polar(next.polar);

      // As far as the control asked, or as far as terrain allows.
      const from = state.view_state;
      const reached = allowed_step(next.dist, next.polar, next.azimuth, from.dist, from.polar, from.azimuth);
      next.dist = norm_dist(reached.dist);
      next.azimuth = norm_azimuth(reached.azimuth);
      next.polar = norm_polar(reached.polar);

      const same: boolean = state.view_state.dist === next.dist &&
        state.view_state.azimuth === next.azimuth &&
        state.view_state.polar === next.polar;

      return same ? state : { view_state: next };
    }),

  reset_dist: () =>
    set((state) => {
      const dist: f64 = norm_dist(DEFAULT_DIST);
      if (state.view_state.dist === dist) return state;
      const from = state.view_state;
      const reached = allowed_step(dist, from.polar, from.azimuth, from.dist, from.polar, from.azimuth);
      const reached_dist = norm_dist(reached.dist);
      return from.dist === reached_dist ? state : { view_state: { ...from, dist: reached_dist } };
    }),
  // azimuth-only: "align north" is what both the dedicated compass button
  // and the follow-mode-off action actually mean — resetting
  // polar (tilt) alongside it used to also flatten/steepen whatever view
  // angle the user had deliberately set, on a click whose entire purpose was
  // "point me north," not "also forget my tilt."
  reset_azimuth: () =>
    set((state) => {
      const azimuth: f64 = norm_azimuth(DEFAULT_AZIMUTH);
      if (state.view_state.azimuth === azimuth) return state;
      const from = state.view_state;
      const reached = allowed_step(from.dist, from.polar, azimuth, from.dist, from.polar, from.azimuth);
      const reached_azimuth = norm_azimuth(reached.azimuth);
      return from.azimuth === reached_azimuth ? state : { view_state: { ...from, azimuth: reached_azimuth } };
    }),
  reset_view: () =>
    set((state) => {
      const next: ViewState = {
        dist: norm_dist(DEFAULT_DIST),
        azimuth: norm_azimuth(DEFAULT_AZIMUTH),
        polar: norm_polar(DEFAULT_POLAR),
      };
      const from = state.view_state;
      const reached = allowed_step(next.dist, next.polar, next.azimuth, from.dist, from.polar, from.azimuth);
      return {
        view_state: {
          dist: norm_dist(reached.dist),
          azimuth: norm_azimuth(reached.azimuth),
          polar: norm_polar(reached.polar),
        },
      };
    }),

  is_follow_mode: false,
  set_is_follow_mode: (is_follow_mode) => set((state) => state.is_follow_mode === is_follow_mode ? state : { is_follow_mode }),
}));
