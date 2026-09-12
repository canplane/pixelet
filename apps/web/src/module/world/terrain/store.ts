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

type TileInterest = {
  matches: (page_idx: IVec) => boolean;
  notify: () => void;
};

/**
 * Live registrations, kept outside zustand state — this is dispatch wiring,
 * not data, so registering/unregistering shouldn't itself trigger a
 * re-render of every store subscriber.
 */
const interests = new Set<TileInterest>();

/**
 * Registers interest in a subset of pages (whichever `matches` returns true
 * for), calling `notify` only when a matching page is set/cleared — instead
 * of every consumer waking on every page load anywhere in the loaded world
 * and re-deriving relevance itself. Returns an unregister function.
 */
export function register_tile_interest(matches: (page_idx: IVec) => boolean, notify: () => void): () => void {
  const interest: TileInterest = { matches, notify };
  interests.add(interest);
  return () => interests.delete(interest);
}

function notify_interests(page_idx: IVec): void {
  for (const interest of interests) {
    if (interest.matches(page_idx)) interest.notify();
  }
}

/** Every resident terrain page. Walk it or look a page up here; change it
 * through the store so React and the interests hear. */
export const tile_pages = new PageTable<Tile>();

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
    notify_interests(page_idx);
  },
  clear_tile: (page_idx) => {
    if (tile_pages.clear(page_idx) === undefined) return;
    set({ revision: tile_pages.revision });
    notify_interests(page_idx);
  },
}));
