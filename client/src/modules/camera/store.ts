// @pxl/web/modules/camera/store.ts

import { create } from "zustand";

import type { f64 } from "@pixelet/shared/kit/word.ts";
import { clamp, round_to_step, wrap } from "@pixelet/shared/kit/math.ts";

import type { PVF3 } from "@pixelet/shared/spec/space/pvf3/mod.ts";

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

  /** User/system-facing camera view intent. */
  view_state: ViewState;
  set_view_state: (partial: Partial<ViewState>) => void;

  // ---- Control Signals ----

  /** Reset signals for camera controllers. */
  reset_dist: () => void;
  reset_orientation: () => void;
  reset_view: () => void;

  // ---- Interaction ----

  /** Follow mode flag. */
  is_follow_mode: boolean;
  set_is_follow_mode: (is_follow_mode: boolean) => void;

  /** When enabled, panning re-samples terrain elevation under the map center every drag step. */
  follow_terrain: boolean;
  set_follow_terrain: (follow_terrain: boolean) => void;
  toggle_follow_terrain: () => void;
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

      const same: boolean = state.view_state.dist === next.dist &&
        state.view_state.azimuth === next.azimuth &&
        state.view_state.polar === next.polar;

      return same ? state : { view_state: next };
    }),

  reset_dist: () =>
    set((state) => {
      const dist: f64 = norm_dist(DEFAULT_DIST);
      return state.view_state.dist === dist ? state : { view_state: { ...state.view_state, dist } };
    }),
  reset_orientation: () =>
    set((state) => {
      const azimuth: f64 = norm_azimuth(DEFAULT_AZIMUTH);
      const polar: f64 = norm_polar(DEFAULT_POLAR);
      const same: boolean = state.view_state.azimuth === azimuth && state.view_state.polar === polar;
      return same ? state : { view_state: { ...state.view_state, azimuth, polar } };
    }),
  reset_view: () =>
    set({
      view_state: {
        dist: norm_dist(DEFAULT_DIST),
        azimuth: norm_azimuth(DEFAULT_AZIMUTH),
        polar: norm_polar(DEFAULT_POLAR),
      },
    }),

  is_follow_mode: false,
  set_is_follow_mode: (is_follow_mode) => set((state) => state.is_follow_mode === is_follow_mode ? state : { is_follow_mode }),

  follow_terrain: true,
  set_follow_terrain: (follow_terrain) => set((state) => state.follow_terrain === follow_terrain ? state : { follow_terrain }),
  toggle_follow_terrain: () => set((state) => ({ follow_terrain: !state.follow_terrain })),
}));
