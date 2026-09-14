// @pxl/web/module/world/poi/store.ts

/**
 * Page cache for POI, keyed by L3 page idx — POI's own counterpart to
 * terrain/store.ts's `useTileStore`, deliberately simpler: no interests
 * registry, since poi_layer.tsx (this directory) is the only consumer today
 * (terrain's own registry exists because many independent readers —
 * elevation lookups, terrain rendering, hover — each care about a different
 * subset of loaded pages; a single reader has nothing to save by
 * fine-graining notification). If a second real consumer shows up, lift
 * this to match terrain/store.ts's shape then, not before.
 */

import { create } from "zustand";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { PageTable } from "@/module/world/page_table.ts";
import { WorldTopic } from "@/module/world/change.ts";
import { release_poi_page, type ResidentPoiPage } from "./repository.ts";

/** Every resident POI page: the key index in the core memory, the page —
 * its packet in a core block, opened as lanes — as its handle. */
export const poi_pages = new PageTable<ResidentPoiPage>(WorldTopic.POI_PAGE);

export type PoiStore = {
  /** poi_pages.revision, mirrored so a selector sees a page come and go. */
  revision: u32;

  get_pois: (page_idx: IVec) => ResidentPoiPage | undefined;
  set_pois: (page_idx: IVec, page: ResidentPoiPage) => void;
  clear_pois: (page_idx: IVec) => void;
};

export const usePoiStore = create<PoiStore>()((set) => ({
  revision: 0,

  get_pois: (page_idx) => poi_pages.get(page_idx),
  set_pois: (page_idx, page) => {
    poi_pages.set(page_idx, page);
    set({ revision: poi_pages.revision });
  },
  clear_pois: (page_idx) => {
    const held = poi_pages.clear(page_idx);
    if (held === undefined) return;
    release_poi_page(held);
    set({ revision: poi_pages.revision });
  },
}));
