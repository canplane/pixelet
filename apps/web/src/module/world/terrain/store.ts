// @pxl/web/module/world/terrain/store.ts

/**
 * Central page cache keyed by L3 page idx.
 *
 * Owns:
 * - coarse page payload storage
 * - page replacement / eviction
 *
 * Does not own:
 * - page idx derivation
 * - fine-grained merge
 * - render / stream scheduling
 *
 * The pages live in `tile_pages` (page_table.ts): the key index in the core
 * memory, the page's typed views as handles. The zustand store carries the
 * table's revision for React, and the three calls every non-React reader
 * already used.
 */

import { create } from "zustand";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { Tile } from "@pxl/shared/bake/format.ts";

import { PageTable } from "@/module/world/page_table.ts";
import { world_wakers, WorldTopic } from "@/module/world/change.ts";

/**
 * Registers interest in a subset of pages (whichever `matches` returns true
 * for), calling `notify` when a matching page is set or cleared. A filter
 * over the world change log's terrain topic, heard at the tick's pump rather
 * than inside the store's write; when the pump lost its place, every
 * resident page is offered to `matches` again. Returns an unregister
 * function.
 */
export function register_tile_interest(matches: (page_idx: IVec) => boolean, notify: (page_idx: IVec) => void): () => void {
  return world_wakers.subscribe_topic(WorldTopic.TERRAIN_PAGE, (key) => {
    if (key === null) {
      for (const page_idx of tile_pages.keys()) if (matches(page_idx)) notify(page_idx);
      return;
    }
    const page_idx = BigInt.asUintN(64, key);
    if (matches(page_idx)) notify(page_idx);
  });
}

/** Every resident terrain page. Walk it or look a page up here; change it
 * through the store so React and the change log hear. */
export const tile_pages = new PageTable<Tile>(WorldTopic.TERRAIN_PAGE);

export type TileStore = {
  /** tile_pages.revision, mirrored so a selector sees a page come and go. */
  revision: u32;

  get_tile: (page_idx: IVec) => Tile | undefined;
  set_tile: (page_idx: IVec, tile: Tile) => void;
  clear_tile: (page_idx: IVec) => void;
};

export const useTileStore = create<TileStore>()((set) => ({
  revision: 0,

  get_tile: (page_idx) => tile_pages.get(page_idx),
  set_tile: (page_idx, tile) => {
    tile_pages.set(page_idx, tile);
    set({ revision: tile_pages.revision });
  },
  clear_tile: (page_idx) => {
    if (tile_pages.clear(page_idx) === undefined) return;
    set({ revision: tile_pages.revision });
  },
}));
