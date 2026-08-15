// @pxl/web/modules/world/terrain/use_tile_fetcher.ts

import { useEffect, useMemo } from "react";

import type { i32, u32 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

import { decode_terrain, type Terrain, type Tile } from "@pxl/shared/bake/format.ts";
import { useTileStore } from "./store.ts";
import { CLEANUP_RADIUS, LOAD_RADIUS } from "./config.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use_map_navigation.ts";

export const CHEBYSHEV_DIST = (v: IVec): u32 => Number(ivec.max_comp(ivec.abs(v)));

const pending_page_demands = new Set<IVec>();

/** Fetches a single terrain page over plain HTTP and stores the decoded result. */
async function fetch_tile(page_idx: IVec): Promise<void> {
  const set_tile = useTileStore.getState().set_tile;

  try {
    const res = await fetch(`/api/world/terrain/tile/${page_idx.toString()}`);
    if (!res.ok) throw new Error(`[tile/${ivec.repr(page_idx)}] http ${res.status}`);

    const _tile = new Uint8Array(await res.arrayBuffer());

    const terrain: Terrain = decode_terrain(_tile);
    const tile: Tile = { terrain };
    // logs only the id, not the decoded tile itself — that's a full terrain
    // payload (elevation grid + feature/coastline rows), and this fires on
    // every tile load, including rapid-fire ones during a pan/zoom gesture;
    // handing the whole object to console.debug on each one is a real,
    // avoidable cost (both the reference retention and however the
    // devtools console chooses to format/inspect it), for a debug line
    // nothing downstream actually reads.
    console.debug(`[tile/${ivec.repr(page_idx)}] loaded`);
    set_tile(page_idx, tile);
  } catch (err) {
    console.error(`[tile/${ivec.repr(page_idx)}] failed to fetch tile:`, err);
  } finally {
    pending_page_demands.delete(page_idx);
  }
}

/**
 * Builds a page demand queue ordered by Chebyshev distance from the center page.
 *
 * `pinned_page_idx` (the device's own page, when it falls outside the normal
 * LOAD_RADIUS window around the view center) is appended unconditionally so
 * the location the user marker grounds itself against is never starved just
 * because the camera has panned away from it — see the doc comment on
 * useTileFetcher for the bug this guards against.
 */
function schedule_page_demands(center_page_idx: IVec, pinned_page_idx: IVec | null): IVec[] {
  const get_tile = useTileStore.getState().get_tile;

  const groups: IVec[][] = Array.from({ length: LOAD_RADIUS + 1 }, () => []); // grouped by distance

  let cnt: u32 = 0;
  for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
    for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
      const page_idx_delta = ivec.gather(BigInt(dx), BigInt(dy), BigInt(0));
      const page_idx = ivec.add(center_page_idx, page_idx_delta);

      // skip pages that are already loaded or already queued.
      if (get_tile(page_idx) || pending_page_demands.has(page_idx)) continue;
      pending_page_demands.add(page_idx);

      // chebyshev distance over the square page neighborhood.
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

  if (
    pinned_page_idx !== null &&
    !get_tile(pinned_page_idx) &&
    !pending_page_demands.has(pinned_page_idx)
  ) {
    pending_page_demands.add(pinned_page_idx);
    flatten.push(pinned_page_idx);
  }

  return flatten;
}

/** Evicts cached pages that fall outside the cleanup radius. */
function cleanup_far_tiles(center_page_idx: IVec, pinned_page_idx: IVec | null): void {
  const { tile_map, clear_tile } = useTileStore.getState();

  // scan the current cache and evict pages beyond the hysteresis radius.
  // the pinned (device-own) page is exempt: it's kept warm regardless of
  // camera distance so the user marker can always ground itself.
  for (const [page_idx] of tile_map) {
    if (pinned_page_idx !== null && page_idx === pinned_page_idx) continue;

    const dist: u32 = CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx));
    if (dist > CLEANUP_RADIUS) {
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
    return L3_IDX(map_center.i);
  }, [map_center]);

  const device_page_idx: IVec | null = useMemo(() => {
    if (device_coord === null) return null;
    return L3_IDX(device_coord.i);
  }, [device_coord]);

  // main demand/update loop.
  useEffect(() => {
    if (center_page_idx === null) return;

    cleanup_far_tiles(center_page_idx, device_page_idx);

    const demands: IVec[] = schedule_page_demands(center_page_idx, device_page_idx);
    // nothing new to fetch (already-cached center, or a trivial re-run) —
    // leave map_status exactly as it is rather than flipping it to SUCCESS
    // on every effect re-run regardless of whether anything actually
    // loaded; that would flash the "sync complete" toast on e.g. every
    // minor pan, not just real load events.
    if (demands.length === 0) return;

    const set_map_status = useMapStore.getState().set_map_status;
    set_map_status(MapStatus.LOADING);

    Promise.all(demands.map((page_idx) => fetch_tile(page_idx))).then(() => {
      // pending_page_demands is module-level, shared across every call to
      // this effect — a later pan can kick off a fresh, still-loading batch
      // before this one's promises settle, and this batch finishing
      // shouldn't stomp SUCCESS over that newer batch's LOADING.
      if (pending_page_demands.size === 0) {
        useMapStore.getState().set_map_status(MapStatus.SUCCESS);
      }
    });
  }, [center_page_idx, device_page_idx]);
}
