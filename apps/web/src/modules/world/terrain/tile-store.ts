// @pxl/web/modules/world/terrain/tile-store.ts

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

import type { IVec } from "@pxl/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/spec/space/xy.ts";

import type { Tile } from "@pxl/bake/terrain/format.ts";

export type TileState = {
  tile_map: Map<IVec, Tile>;

  get_tile: (page_idx: IVec) => Tile | undefined;
  set_tile: (page_idx: IVec, tile: Tile) => void;
  clear_tile: (page_idx: IVec) => void;
};

export const useTileStore = create<TileState>()(
  immer((set, get) => ({
    tile_map: new Map<IVec, Tile>(),

    get_tile: (page_idx) => get().tile_map.get(TO_XY(page_idx)),
    set_tile: (page_idx, tile) => set((state) => void state.tile_map.set(TO_XY(page_idx), tile)),
    clear_tile: (page_idx) => set((state) => void state.tile_map.delete(TO_XY(page_idx))),
  })),
);
