// @pxl/web/module/world/terrain/use_tile_fetcher.ts

import { useEffect, useMemo } from "react";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import type { Demand, Phases } from "@/module/world/sched/demand.ts";
import { fetch_pending_count, fetch_running, fetch_subscribe, fetch_sync } from "@/module/world/sched/fetch_scheduler.ts";
import { fetch_terrain } from "./tile_repository.ts";
import { release_terrain } from "./resident.ts";
import { tile_pages, useTileStore } from "./store.ts";
import { useRangeAnchor } from "@/module/world/frame/use_range_anchor.ts";
import { FETCH_HYSTERESIS, page_priority, pages_to_hold } from "@/module/world/frame/range.ts";
import { KEEP_DIST, RESIDENT_PAGES } from "./config.ts";
import { MapStatus, useMapStore } from "@/module/map/store.ts";
import { useMapNavigation } from "@/module/map/navigation/use_map_navigation.ts";

const CATEGORY = "terrain";

// The device's own page, when it's outside the fetch range, still has
// to load so the user marker can ground itself — but it's genuinely far, so
// it queues behind everything near the camera rather than jumping the line.
const PINNED_PAGE_PRIORITY = 1_000;

/** Builds the terrain wishlist: one Demand per uncached page within
 * the held set, nearest first, plus the device page when it falls outside
 * that set. The scheduler dedups against what it already
 * holds, so this is rebuilt in full on every effect run. Both page indices
 * arrive already TO_XY-normalized (the effect's own useMemos), matching the
 * keys tile store and scheduler use. */
function terrain_demands(anchor: IVec, pinned_page_idx: IVec | null): Demand[] {
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

  const held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES);
  for (const { page_idx, dist2 } of held) add(page_idx, page_priority(dist2));
  // A request already running for a page that has only just left the held
  // set is kept in the wishlist rather than withdrawn, so a pan does not
  // cancel work it will ask for again — see FETCH_HYSTERESIS.
  const held_keys = new Set(held.map(({ page_idx }) => page_idx));
  for (const { page_idx, dist2 } of pages_to_hold(anchor, KEEP_DIST + FETCH_HYSTERESIS, RESIDENT_PAGES + 16)) {
    if (!held_keys.has(page_idx) && fetch_running(`${CATEGORY}:${page_idx}`)) add(page_idx, page_priority(dist2));
  }

  if (pinned_page_idx !== null) add(pinned_page_idx, PINNED_PAGE_PRIORITY);

  return demands;
}

/** Evicts cached pages that fall outside the cleanup radius. */
function cleanup_far_tiles(anchor: IVec, pinned_page_idx: IVec | null): void {
  const { clear_tile } = useTileStore.getState();

  // Exactly the set the demands above ask for, so a page is never evicted
  // while it is still being fetched. The pinned (device-own) page is exempt:
  // it is kept warm regardless of camera distance so the user marker can
  // always ground itself.
  const held = new Set(pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES).map(({ page_idx }) => page_idx));

  // Collected first: clearing moves the table's last page into the hole.
  const evict: IVec[] = [];
  for (const page_idx of tile_pages.keys()) {
    if (page_idx === pinned_page_idx || held.has(page_idx)) continue;
    evict.push(page_idx);
  }
  for (const page_idx of evict) {
    release_terrain(tile_pages.get(page_idx)!.terrain);
    clear_tile(page_idx);
  }
}

export function useTileFetcher() {
  // the device's own page has to stay loaded independently of the view
  // center: panning/free-look moves map_center away from the user's real
  // GPS position, and without this, that page falls outside the held set,
  // get_ele() for the device's own
  // coordinate never resolves again, and the user marker silently vanishes
  // until the camera happens to pan back near it.
  const { device_coord } = useMapNavigation();

  const anchor = useRangeAnchor();

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
    if (anchor === null) return;

    cleanup_far_tiles(anchor, device_page_idx);
    fetch_sync(CATEGORY, terrain_demands(anchor, device_page_idx));
  }, [anchor, device_page_idx]);
}
