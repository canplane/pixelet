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
import { useMapStore } from "@/modules/map/navigation/map-store.ts";

/** Runtime tuning constants for geo streaming / rendering policy. */

// page preload radius around current center page
const LOAD_RADIUS: u32 = 2;
// cache eviction radius with hysteresis
const CLEANUP_RADIUS: u32 = Math.round(LOAD_RADIUS * 1.5);

const CHEBYSHEV_DIST = (v: IVec): u32 => Number(ivec.max_comp(ivec.abs(v)));

const deferred_demands = new Set<IVec>();

function register() {
  const { post } = get_xpute();
  const { set_tile } = useTileStore.getState();

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
      deferred_demands.delete(page_idx);
    }
  });
  return unsub;
}

/**
 * Scheduling Queue Generator
 * 현재 위치 기준 로딩이 필요한 페이지들을 거리순으로 정렬하여 반환합니다.
 */
function schedule(center_id: IVec): IVec[] {
  const { get_tile } = useTileStore.getState();

  const groups: IVec[][] = Array.from({ length: LOAD_RADIUS + 1 }, () => []); // 거리별 그룹핑

  let cnt: u32 = 0;
  for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
    for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
      const page_idx_delta = ivec.gather(BigInt(dx), BigInt(dy), BigInt(0));
      const page_idx = ivec.add(center_id, page_idx_delta);

      // 이미 로드되었거나, 현재 로딩 중(Processing)인 페이지는 스킵
      if (get_tile(page_idx) || deferred_demands.has(page_idx)) continue;
      deferred_demands.add(page_idx);

      // Chebyshev distance (정사각 그리드 로딩)
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

/**
 * Cleanup Strategy
 * 범위 밖 페이지를 정리합니다.
 */
function cleanup(center_id: IVec): void {
  const { tile_map, clear_tile } = useTileStore.getState();

  // Store의 Map을 직접 순회하며 거리 계산
  for (const [page_idx] of tile_map) {
    const dist: u32 = CHEBYSHEV_DIST(ivec.sub(page_idx, center_id));
    if (dist > CLEANUP_RADIUS) {
      clear_tile(page_idx); // 1.5배 범위 밖: 완전 제거
    }
  }
}

export function useTileFetcher() {
  const { post } = get_xpute();
  const map_center = useMapStore((state) => state.map_center);

  const center_id: IVec | null = useMemo(() => {
    if (map_center === null) return null;
    return L3_IDX(map_center.i);
  }, [map_center]);

  useEffect(() => {
    const unsub = register();
    return unsub;
  }, []);

  // Main Loop
  useEffect(() => {
    if (center_id === null) return;

    cleanup(center_id);

    const demands: IVec[] = schedule(center_id); // create request queue
    if (demands.length > 0) {
      void post.req.unack().send(Command.TILE, demands, null); // batch request
    }
  }, [center_id]);
}
