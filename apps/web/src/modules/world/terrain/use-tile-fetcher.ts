// @pxl/web/modules/world/terrain/use-tile-fetcher.ts

import { useEffect, useMemo } from "react";

import type { i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { U8Array } from "@xpute/spec/lib/kit/array.ts";
import { get_xpute } from "@/xpute/context.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/spec/space/indexer.ts";

import { Command } from "@xpute/spec/control/cmd.ts";
import { decode_terrain, type Terrain, type Tile } from "@pxl/bake/terrain/format.ts";
import { useTileStore } from "./tile-store.ts";
import { useMapStore } from "@/modules/map/store.ts";

/** Runtime tuning constants for terrain page streaming. */

// Preload radius around the current center page.
const LOAD_RADIUS: u32 = 2;
// Cache eviction radius with hysteresis.
const CLEANUP_RADIUS: u32 = Math.round(LOAD_RADIUS * 2);

const CHEBYSHEV_DIST = (v: IVec): u32 => Number(ivec.max_comp(ivec.abs(v)));

const pending_page_demands = new Set<IVec>();

function register_tile_stream() {
  const { post } = get_xpute();
  const set_tile = useTileStore.getState().set_tile;

  const unsub = post.on<[IVec]>(Command.TILE, (args, body) => {
    const [page_idx] = args;

    try {
      if (!body) throw new Error(`[${Command.TILE}/${ivec.repr(page_idx)}] shell: tile is empty`);
      const _tile: U8Array = body;

      const terrain: Terrain = decode_terrain(_tile);
      const tile: Tile = { terrain };
      console.debug(`[${Command.TILE}/${ivec.repr(page_idx)}] shell:`, tile);
      set_tile(page_idx, tile);
    } catch (err) {
      console.error(`[${Command.TILE}/${ivec.repr(page_idx)}] shell: failed to stream tile:`, err);
    } finally {
      pending_page_demands.delete(page_idx);
    }
  });
  return unsub;
}

/**
 * Builds a page demand queue ordered by Chebyshev distance from the center page.
 */
function schedule_page_demands(center_page_idx: IVec): IVec[] {
  const get_tile = useTileStore.getState().get_tile;

  const groups: IVec[][] = Array.from({ length: LOAD_RADIUS + 1 }, () => []); // 거리별 그룹핑

  let cnt: u32 = 0;
  for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
    for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
      const page_idx_delta = ivec.gather(BigInt(dx), BigInt(dy), BigInt(0));
      const page_idx = ivec.add(center_page_idx, page_idx_delta);

      // Skip pages that are already loaded or already queued.
      if (get_tile(page_idx) || pending_page_demands.has(page_idx)) continue;
      pending_page_demands.add(page_idx);

      // Chebyshev distance over the square page neighborhood.
      const dist: i32 = CHEBYSHEV_DIST(page_idx_delta);
      groups[dist].push(page_idx), cnt++;
    }
  }

  const flatten: IVec[] = new Array(cnt);

  let i: u32 = 0;
  for (let d = 0; d < groups.length; d++) {
    const group: IVec[] = groups[d];
    for (let j = 0; j < group.length; j++) {
      flatten[i++] = group[j];
    }
  }
  return flatten;
}

/** Evicts cached pages that fall outside the cleanup radius. */
function cleanup_far_tiles(center_page_idx: IVec): void {
  const { tile_map, clear_tile } = useTileStore.getState();

  // Scan the current cache and evict pages beyond the hysteresis radius.
  for (const [page_idx] of tile_map) {
    const dist: u32 = CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx));
    if (dist > CLEANUP_RADIUS) {
      clear_tile(page_idx);
    }
  }
}

export function useTileFetcher() {
  const { post } = get_xpute();
  const map_center = useMapStore((state) => state.map_center);

  const center_page_idx: IVec | null = useMemo(() => {
    if (map_center === null) return null;
    return L3_IDX(map_center.i);
  }, [map_center]);

  useEffect(() => {
    const unsub = register_tile_stream();
    return unsub;
  }, []);

  // Main demand/update loop.
  useEffect(() => {
    if (center_page_idx === null) return;

    cleanup_far_tiles(center_page_idx);

    const demands: IVec[] = schedule_page_demands(center_page_idx);
    if (demands.length > 0) {
      void post.req.unack().send(Command.TILE, demands, null);
    }
  }, [center_page_idx]);
}
