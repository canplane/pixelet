// @pxl/web/app/settings/debug_store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * Dev-only diagnostic toggles — __IS_DEV__-gated, never shipped visible in
 * production (see settings_store.ts for the user-facing settings that used
 * to live here alongside these, lock_camera_tilt included).
 */
export type DebugStore = {
  /**
   * Runtime toggle for the dev-only debug gizmos (frame-origin spheres, the
   * map_center/map_origin dots, page-corner blocks — everything gated by
   * `__IS_DEV__` in world.tsx's DebugLayer). Defaults to false: these are
   * opt-in diagnostics, not something that should be in the way while
   * eyeballing real rendering behavior.
   */
  showDebugObjects: boolean;
  setShowDebugObjects: (show: boolean) => void;
  toggleDebugObjects: () => void;
};

// Persisted (localStorage) — same reasoning as settings_store.ts's own
// persist wrap: a dev toggle left on shouldn't silently revert on reload.
export const useDebugStore = create<DebugStore>()(persist((set) => ({
  showDebugObjects: false,
  setShowDebugObjects: (showDebugObjects) => set({ showDebugObjects }),
  toggleDebugObjects: () => set((state) => ({ showDebugObjects: !state.showDebugObjects })),
}), { name: "pxl:debug_settings" }));
