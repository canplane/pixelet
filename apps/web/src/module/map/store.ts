// @pxl/web/module/map/store.ts

import { create } from "zustand";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

/** Data loading lifecycle. */
export const enum MapStatus {
  IDLE = "idle",
  LOADING = "loading",
  SUCCESS = "success",
  ERROR = "error",
}

export type MapStore = {
  // ---- Lifecycle & Status ----

  map_status: MapStatus;
  set_map_status: (status: MapStatus) => void;

  // ---- Navigation & Coordinate ----

  /** Canonical absolute target. */
  map_center: PVF3 | null;
  set_map_center: (map_center: PVF3) => void;
  // ---- Interaction ----

  /** HTML portal mount point for canvas overlays. */
};

export const useMapStore = create<MapStore>((set, _get) => ({
  map_status: MapStatus.IDLE,
  set_map_status: (map_status) => set({ map_status }),

  map_center: null,
  /** Stores absolute PVF3 without rebasing/localization. */
  set_map_center: (map_center) => set({ map_center }),
}));
