// @pxl/web/modules/map/navigation/map-store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/bake/terrain/model/osm_feature.ts";

import { CameraControlMode, render_cfg } from "@/modules/world/render/config.ts";

const { DEFAULT_POLAR_ANGLE } = render_cfg.camera;

/** Data loading lifecycle. */
export const enum MapDataStatus {
  IDLE = "idle",
  LOADING = "loading",
  SUCCESS = "success",
  ERROR = "error",
}

/** Camera view intent. */
export type ViewState = {
  dist: f64; // 줌 레벨 (카메라-타겟 거리)
  bearing: f64; // 수평 회전 각도 (0~360, 북쪽 기준)
  polar_angle: f64; // 수직 각도 (0:수직, PI/2:수평)
  height: f64; // 지면으로부터의 카메라 수직 높이
};

/**
 * Canonical geo app state.
 *
 * Owns:
 * - center
 * - camera / follow / hover / selection intent
 * - coarse UI lifecycle state
 *
 * Does not own:
 * - origin
 * - local/render coordinates
 * - page/chunk indices
 */
export type MapState = {
  // ---- Lifecycle & Status ----

  status: MapDataStatus;
  set_status: (status: MapDataStatus) => void;

  camera_control_mode: CameraControlMode;
  set_camera_control_mode: (camera_control_mode: CameraControlMode) => void;

  // ---- Navigation & Coordinate ----

  /** Canonical absolute target. */
  map_center: PVF3 | null;
  set_map_center: (center: PVF3) => void;

  /** Camera view intent. */
  view_state: ViewState;
  set_view_state: (view: Partial<ViewState>) => void;

  /** Camera flight flag. */
  is_flying: boolean;
  set_is_flying: (is_flying: boolean) => void;

  // ---- Control Signals ----

  /** Reset signals for camera controllers. */
  camera_reset_signal: { dist: f64; orientation: f64 };
  trigger_dist_reset: () => void;
  trigger_orientation_reset: () => void;

  // ---- Interaction ----

  /** Hovered canonical coordinate from raycast. */
  hovered_coord: PVF3 | null;
  set_hovered_coord: (coord: PVF3 | null) => void;

  /** Selected POI entity. */
  selected_poi: POI | null;
  set_selected_poi: (poi: POI | null) => void;

  /** HTML portal mount point for canvas overlays. */
  html_portal_el: HTMLDivElement | null;
  set_html_portal_el: (el: HTMLDivElement | null) => void;

  /** Follow mode flag. */
  is_follow_mode: boolean;
  set_is_follow_mode: (is_follow_mode: boolean) => void;
};

/** Canonical geo global state. */
export const useMapStore = create<MapState>((set, get) => ({
  status: MapDataStatus.IDLE,
  set_status: (status) => set({ status }),

  camera_control_mode: CameraControlMode.NONE,
  set_camera_control_mode: (camera_control_mode: CameraControlMode) => set({ camera_control_mode }),

  map_center: null,
  /** Stores absolute PVF3 without rebasing/localization. */
  set_map_center: (map_center: PVF3) => set({ map_center, camera_control_mode: CameraControlMode.SYSTEM }),

  view_state: {
    dist: 200,
    bearing: 0,
    polar_angle: DEFAULT_POLAR_ANGLE,
    height: 100,
  },
  set_view_state: (view) =>
    set((state) => ({
      view_state: { ...state.view_state, ...view },
    })),

  is_flying: false,
  /** Mode restoration is owned by camera/orbit controllers. */
  set_is_flying: (is_flying) => set({ is_flying }),

  camera_reset_signal: { dist: 0, orientation: 0 },
  trigger_dist_reset: () =>
    set((state) => ({
      camera_reset_signal: { ...state.camera_reset_signal, dist: Date.now() },
    })),
  trigger_orientation_reset: () =>
    set((state) => ({
      camera_reset_signal: {
        ...state.camera_reset_signal,
        orientation: Date.now(),
      },
    })),

  hovered_coord: null,
  set_hovered_coord: (coord) => set({ hovered_coord: coord }),

  selected_poi: null,
  set_selected_poi: (poi) => set({ selected_poi: poi }),

  html_portal_el: null,
  set_html_portal_el: (el) => set({ html_portal_el: el }),

  is_follow_mode: false,
  set_is_follow_mode: (is_follow_mode) => set({ is_follow_mode }),
}));
