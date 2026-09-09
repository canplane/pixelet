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

  /**
   * Cube edge shading — knobs, not a feature. The old fake bevel darkened
   * every cube edge unconditionally, which drew a grid across flat ground
   * and read as artificial; it now darkens only edges that border open air,
   * which chunk_mesher.ts's face mask already identifies (an edge of a top
   * face is a silhouette exactly when the side face in that direction
   * survived culling). Whether that reads as "the terrain has height" or as
   * "the terrain has outlines" is a judgement to make by eye, so the three
   * numbers behind it are adjustable until one set is chosen and frozen.
   *
   * `edgeShadeStrength` 0 turns it off entirely; the defaults are what read
   * best by eye once only silhouette and concave edges were shaded, not the
   * old bevel's numbers, which were tuned when every edge was.
   * `edgeShadeWidth` is in world units.
   */
  edgeShadeStrength: number;
  edgeShadeWidth: number;
  setEdgeShade: (next: Partial<{ edgeShadeStrength: number; edgeShadeWidth: number }>) => void;
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

  edgeShadeStrength: 0.3,
  edgeShadeWidth: 0.03,
  setEdgeShade: (next) => set(next),
}), { name: "pxl:debug_settings" }));
