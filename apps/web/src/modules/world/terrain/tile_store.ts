// @pxl/web/modules/world/terrain/tile_store.ts

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
 */

import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import type { Tile } from "@pxl/shared/bake/format.ts";

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

export type TileStore = {
  tile_map: Map<IVec, Tile>;

  get_tile: (page_idx: IVec) => Tile | undefined;
  set_tile: (page_idx: IVec, tile: Tile) => void;
  clear_tile: (page_idx: IVec) => void;
};

export const useTileStore = create<TileStore>()(
  immer((set, get) => ({
    tile_map: new Map<IVec, Tile>(),

    get_tile: (page_idx) => get().tile_map.get(TO_XY(page_idx)),
    set_tile: (page_idx, tile) => {
      set((state) => {
        state.tile_map.set(TO_XY(page_idx), tile);
      });
      notify_interests(page_idx);
    },
    clear_tile: (page_idx) => {
      set((state) => {
        state.tile_map.delete(TO_XY(page_idx));
      });
      notify_interests(page_idx);
    },
  })),
);
