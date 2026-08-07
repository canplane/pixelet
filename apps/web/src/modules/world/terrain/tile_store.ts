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

export type TileStore = {
  tile_map: Map<IVec, Tile>;

  /**
   * Bumped on every set_tile/clear_tile. `tile_map` itself is a plain Map
   * mutated in place (via immer's draft) rather than replaced, so its
   * identity never changes — nothing that just does `useTileStore(state =>
   * state.tile_map)` re-renders when a page streams in. Consumers that need
   * to react to "some tile finished loading" in general (not one specific
   * page_idx via get_tile) — e.g. UserMarker's zone ring, which samples
   * get_ele() across many neighboring pages once up front and has no other
   * signal telling it a page it originally missed has since arrived —
   * subscribe to this counter instead.
   */
  tile_version: number;

  get_tile: (page_idx: IVec) => Tile | undefined;
  set_tile: (page_idx: IVec, tile: Tile) => void;
  clear_tile: (page_idx: IVec) => void;
};

export const useTileStore = create<TileStore>()(
  immer((set, get) => ({
    tile_map: new Map<IVec, Tile>(),
    tile_version: 0,

    get_tile: (page_idx) => get().tile_map.get(TO_XY(page_idx)),
    set_tile: (page_idx, tile) =>
      set((state) => {
        state.tile_map.set(TO_XY(page_idx), tile);
        state.tile_version++;
      }),
    clear_tile: (page_idx) =>
      set((state) => {
        state.tile_map.delete(TO_XY(page_idx));
        state.tile_version++;
      }),
  })),
);
