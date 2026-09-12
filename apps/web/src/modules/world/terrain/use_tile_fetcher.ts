// @pxl/web/modules/world/terrain/use_tile_fetcher.ts

import { useEffect, useMemo } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import type { Demand, Phases } from "@/modules/world/sched/demand.ts";
import { fetch_pending_count, fetch_subscribe, fetch_sync } from "@/modules/world/sched/fetch_scheduler.ts";
import { fetch_terrain } from "./tile_repository.ts";
import { release_terrain } from "./resident.ts";
import { useTileStore } from "./store.ts";
import { CLEANUP_RADIUS, LOAD_RADIUS } from "./config.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use_map_navigation.ts";

export const CHEBYSHEV_DIST = (v: IVec): u32 => Number(ivec.max_comp(ivec.abs(v)));

const CATEGORY = "terrain";

// The device's own page, when it's outside the LOAD_RADIUS window, still has
// to load so the user marker can ground itself — but it's genuinely far, so
// it queues behind everything near the camera rather than jumping the line.
const PINNED_PAGE_PRIORITY = 1_000;

/** Builds the terrain wishlist: one Demand per uncached page within
 * LOAD_RADIUS, ordered by Chebyshev page distance, plus the device page when
 * it falls outside that window. The scheduler dedups against what it already
 * holds, so this is rebuilt in full on every effect run. Both page indices
 * arrive already TO_XY-normalized (the effect's own useMemos), matching the
 * keys tile store and scheduler use. */
function terrain_demands(center_page_idx: IVec, pinned_page_idx: IVec | null): Demand[] {
  const get_tile = useTileStore.getState().get_tile;
  const demands: Demand[] = [];

  const add = (page_idx: IVec, priority: number) => {
    if (get_tile(page_idx)) return;
    demands.push({
      key: `${CATEGORY}:${page_idx}`,
      category: CATEGORY,
      priority,
      run: async (signal) => {
        const phases: Phases = {};
        const terrain = await fetch_terrain(page_idx, signal, phases);
        useTileStore.getState().set_tile(page_idx, { terrain });
        return phases;
      },
    });
  };

  for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
    for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
      const delta = ivec.gather(BigInt(dx), BigInt(dy), 0n);
      add(ivec.add(center_page_idx, delta), CHEBYSHEV_DIST(delta) * 2);
    }
  }

  if (pinned_page_idx !== null) add(pinned_page_idx, PINNED_PAGE_PRIORITY);

  return demands;
}

/** Evicts cached pages that fall outside the cleanup radius. */
function cleanup_far_tiles(center_page_idx: IVec, pinned_page_idx: IVec | null): void {
  const { tile_map, clear_tile } = useTileStore.getState();

  // scan the current cache and evict pages beyond the hysteresis radius.
  // the pinned (device-own) page is exempt: it's kept warm regardless of
  // camera distance so the user marker can always ground itself.
  for (const [page_idx, tile] of tile_map) {
    if (pinned_page_idx !== null && page_idx === pinned_page_idx) continue;

    const dist: u32 = CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx));
    if (dist > CLEANUP_RADIUS) {
      release_terrain(tile.terrain);
      clear_tile(page_idx);
    }
  }
}

export function useTileFetcher() {
  const map_center = useMapStore((state) => state.map_center);
  // the device's own page has to stay loaded independently of the view
  // center: panning/free-look moves map_center away from the user's real
  // GPS position, and without this, that page falls outside LOAD_RADIUS (or
  // gets evicted past CLEANUP_RADIUS), get_ele() for the device's own
  // coordinate never resolves again, and the user marker silently vanishes
  // until the camera happens to pan back near it.
  const { device_coord } = useMapNavigation();

  const center_page_idx: IVec | null = useMemo(() => {
    if (map_center === null) return null;
    return TO_XY(L3_IDX(map_center.i));
  }, [map_center]);

  const device_page_idx: IVec | null = useMemo(() => {
    if (device_coord === null) return null;
    return TO_XY(L3_IDX(device_coord.i));
  }, [device_coord]);

  // map_status follows the terrain queue: LOADING while anything terrain is
  // queued or in flight, SUCCESS once it drains. Derived from the scheduler
  // rather than tracked here, so a later pan that supersedes this batch
  // can't leave a stale SUCCESS behind.
  useEffect(() => {
    return fetch_subscribe(() => {
      const pending = fetch_pending_count(CATEGORY);
      const { map_status, set_map_status } = useMapStore.getState();
      if (pending > 0) {
        if (map_status !== MapStatus.LOADING) set_map_status(MapStatus.LOADING);
      } else if (map_status === MapStatus.LOADING) {
        set_map_status(MapStatus.SUCCESS);
      }
    });
  }, []);

  // main demand/eviction loop.
  useEffect(() => {
    if (center_page_idx === null) return;

    cleanup_far_tiles(center_page_idx, device_page_idx);
    fetch_sync(CATEGORY, terrain_demands(center_page_idx, device_page_idx));
  }, [center_page_idx, device_page_idx]);
}
