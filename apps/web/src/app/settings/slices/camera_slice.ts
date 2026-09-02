// @pxl/web/app/settings/slices/camera_slice.ts

import type { StateCreator } from "zustand";

/** Fields backing settings/sections/camera_section.tsx. */
export type CameraSettingsSlice = {
  /**
   * When true, camera/input.tsx's rotate-drag and pinch gestures no longer
   * write polar (tilt) — azimuth still works exactly the same either way.
   * The gesture code itself (axis-lock, pinch tracking) is untouched by
   * this; the flag only gates the final set_view_state call each path
   * would otherwise make. Defaults to true — free-tilt-by-gesture was
   * validated (2026-08-30) as UX complexity not worth keeping on by
   * default, not a bug fix, which is why this is a toggle here rather
   * than a deleted code path: easy to flip back on to compare against.
   */
  lock_camera_tilt: boolean;
  set_lock_camera_tilt: (lock: boolean) => void;
  toggle_lock_camera_tilt: () => void;

  /** When enabled, panning re-samples terrain elevation under the map center every drag step. */
  ground_snap: boolean;
  set_ground_snap: (ground_snap: boolean) => void;
  toggle_ground_snap: () => void;

  /**
   * Whether the terrain shader paints the STRUCTURE ground channel, i.e.
   * whether OSM buildings are drawn. The bake writes that channel either way
   * (see osm_policy.ts's determine_cell_cfg), so this is purely a render
   * decision. Still false by default; whether that is the right default is
   * a separate question from where the switch lives.
   */
  show_structures: boolean;
  set_show_structures: (show: boolean) => void;
  toggle_structures: () => void;
};

export const create_camera_settings_slice: StateCreator<CameraSettingsSlice, [], [], CameraSettingsSlice> = (set) => ({
  lock_camera_tilt: true,
  set_lock_camera_tilt: (lock_camera_tilt) => set({ lock_camera_tilt }),
  toggle_lock_camera_tilt: () => set((state) => ({ lock_camera_tilt: !state.lock_camera_tilt })),

  ground_snap: true,
  set_ground_snap: (ground_snap) => set((state) => state.ground_snap === ground_snap ? state : { ground_snap }),
  toggle_ground_snap: () => set((state) => ({ ground_snap: !state.ground_snap })),

  show_structures: false,
  set_show_structures: (show_structures) => set({ show_structures }),
  toggle_structures: () => set((state) => ({ show_structures: !state.show_structures })),
});
