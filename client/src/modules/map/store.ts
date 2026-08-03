// @pxl/web/modules/map/store.ts

import { create } from "zustand";

import type { PVF3 } from "@pixelet/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pixelet/shared/bake/model/osm_feature.ts";

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

  /** Selected POI entity. */
  selected_poi: POI | null;
  set_selected_poi: (poi: POI | null) => void;

  /** HTML portal mount point for canvas overlays. */
  html_portal_el: HTMLDivElement | null;
  set_html_portal_el: (el: HTMLDivElement | null) => void;
};

export const useMapStore = create<MapStore>((set, _get) => ({
  map_status: MapStatus.IDLE,
  set_map_status: (map_status) => set({ map_status }),

  map_center: null,
  /** Stores absolute PVF3 without rebasing/localization. */
  set_map_center: (map_center) => set({ map_center }),

  selected_poi: null,
  set_selected_poi: (poi) => set({ selected_poi: poi }),

  html_portal_el: null,
  set_html_portal_el: (el) => set({ html_portal_el: el }),
}));
