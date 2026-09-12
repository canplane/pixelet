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

import type { u32 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { POI } from "@pxl/shared/bake/model/feature.ts";

import { PageTable } from "@/modules/world/page_table.ts";

/** Every resident POI page: the key index in the core memory, the page's
 * records as its handle. The records themselves are still objects — their
 * lane form is the record format work, not this table's. */
export const poi_pages = new PageTable<readonly POI[]>();

export type PoiStore = {
  /** poi_pages.revision, mirrored so a selector sees a page come and go. */
  revision: u32;

  get_pois: (page_idx: IVec) => readonly POI[] | undefined;
  set_pois: (page_idx: IVec, pois: readonly POI[]) => void;
  clear_pois: (page_idx: IVec) => void;
};

export const usePoiStore = create<PoiStore>()((set) => ({
  revision: 0,

  get_pois: (page_idx) => poi_pages.get(page_idx),
  set_pois: (page_idx, pois) => {
    poi_pages.set(page_idx, pois);
    set({ revision: poi_pages.revision });
  },
  clear_pois: (page_idx) => {
    if (poi_pages.clear(page_idx) === undefined) return;
    set({ revision: poi_pages.revision });
  },
}));
