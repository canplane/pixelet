// @pxl/web/app/dev/debug_store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * Dev-only diagnostic toggles — __IS_DEV__-gated, never shipped visible in
 * production (see render/settings_store.ts for the user-facing
 * rendering-quality settings that used to live here alongside these).
 */
export type DebugStore = {
  /**
   * Runtime toggle for the dev-only debug gizmos (frame-origin spheres, the
   * map_center/map_origin dots, page-corner blocks — everything gated by
   * `__IS_DEV__` in world.tsx's DebugLayer). Defaults to false: these are
   * opt-in diagnostics, not something that should be in the way while
   * eyeballing real rendering behavior.
   */
  show_debug_objects: boolean;
  set_show_debug_objects: (show: boolean) => void;
  toggle_debug_objects: () => void;

  /**
   * Terrain bakes buildings into the STRUCTURE ground channel regardless of
   * this flag (see osm_policy.ts's determine_cell_cfg) — this only controls
   * whether the terrain shader paints that channel (shader.ts's
   * u_show_structures / is_structure). Defaults to false: structures are
   * hidden out of the box, same as the other debug-only visuals here.
   */
  show_structures: boolean;
  set_show_structures: (show: boolean) => void;
  toggle_structures: () => void;
};

// Persisted (localStorage) — same reasoning as render/settings_store.ts's
// own persist wrap: a dev toggle left on shouldn't silently revert on
// reload.
export const useDebugStore = create<DebugStore>()(persist((set) => ({
  show_debug_objects: false,
  set_show_debug_objects: (show_debug_objects) => set({ show_debug_objects }),
  toggle_debug_objects: () => set((state) => ({ show_debug_objects: !state.show_debug_objects })),

  show_structures: false,
  set_show_structures: (show_structures) => set({ show_structures }),
  toggle_structures: () => set((state) => ({ show_structures: !state.show_structures })),
}), { name: "pxl:debug_settings" }));
