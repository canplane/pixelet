// @pxl/web/modules/world/poi/store.ts

/**
 * Page cache for POI, keyed by L3 page idx — POI's own counterpart to
 * terrain/store.ts's `useTileStore`, deliberately simpler: no interests
 * registry, since PoiLayer.tsx (this directory) is the only consumer today
 * (terrain's own registry exists because many independent readers —
 * elevation lookups, terrain rendering, hover — each care about a different
 * subset of loaded pages; a single reader has nothing to save by
 * fine-graining notification). If a second real consumer shows up, lift
 * this to match terrain/store.ts's shape then, not before.
 */

import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

export type PoiStore = {
  poi_map: Map<IVec, POI[]>;

  get_pois: (page_idx: IVec) => POI[] | undefined;
  set_pois: (page_idx: IVec, pois: POI[]) => void;
  clear_pois: (page_idx: IVec) => void;
};

export const usePoiStore = create<PoiStore>()(
  immer((set, get) => ({
    poi_map: new Map<IVec, POI[]>(),

    get_pois: (page_idx) => get().poi_map.get(TO_XY(page_idx)),
    set_pois: (page_idx, pois) => {
      set((state) => {
        state.poi_map.set(TO_XY(page_idx), pois);
      });
    },
    clear_pois: (page_idx) => {
      set((state) => {
        state.poi_map.delete(TO_XY(page_idx));
      });
    },
  })),
);
