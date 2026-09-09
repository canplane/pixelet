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

  /**
   * The sched/ledger.ts dev overlay — per-frame pacing, GPU ask, queue
   * pressure, task latency, LoAF long-frame split. Off by default: it polls
   * the ledger on a timer and paints a corner panel, so it should be opt-in
   * while eyeballing real rendering, same as showDebugObjects.
   */
  showLedgerOverlay: boolean;
  setShowLedgerOverlay: (show: boolean) => void;
  toggleLedgerOverlay: () => void;

  /**
   * Streams sched/ledger.ts records to the console as they land (log.ts) —
   * the text counterpart to the overlay's gauges, for watching a stutter
   * happen rather than reading its settled aggregate. Independent of
   * showLedgerOverlay: either is useful without the other. Off by default,
   * since it writes a console line per settled task.
   */
  logLedger: boolean;
  setLogLedger: (on: boolean) => void;
  toggleLogLedger: () => void;
};

// Persisted (localStorage) — same reasoning as settings_store.ts's own
// persist wrap: a dev toggle left on shouldn't silently revert on reload.
export const useDebugStore = create<DebugStore>()(persist((set) => ({
  showDebugObjects: false,
  setShowDebugObjects: (showDebugObjects) => set({ showDebugObjects }),
  toggleDebugObjects: () => set((state) => ({ showDebugObjects: !state.showDebugObjects })),

  showLedgerOverlay: false,
  setShowLedgerOverlay: (showLedgerOverlay) => set({ showLedgerOverlay }),
  toggleLedgerOverlay: () => set((state) => ({ showLedgerOverlay: !state.showLedgerOverlay })),

  logLedger: false,
  setLogLedger: (logLedger) => set({ logLedger }),
  toggleLogLedger: () => set((state) => ({ logLedger: !state.logLedger })),
}), { name: "pxl:debug_settings" }));
