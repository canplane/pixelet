// @pxl/web/modules/world/poi/use_poi_fetcher.ts

import { useEffect } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import type { Demand, Phases } from "@/modules/world/sched/demand.ts";
import { fetch_sync } from "@/modules/world/sched/fetch_scheduler.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { CLEANUP_RADIUS, LOAD_RADIUS } from "../terrain/config.ts";
import { fetch_poi } from "./repository.ts";
import { usePoiStore } from "./store.ts";

const CATEGORY = "poi";

// POI sits one behind terrain at equal page distance — poi_layer.tsx's
// poi_anchor grounds each badge against that page's terrain, so fetching POI
// ahead of the ground it lands on just makes it wait.
const PRIORITY_BIAS = 1;

/** Evicts cached pages that fall outside CLEANUP_RADIUS, mirroring
 * terrain/use_tile_fetcher.ts's own cleanup_far_tiles — POI has no
 * pinned-page exemption to carry over (that exists there only for the
 * device marker's own grounding, not a concern here). */
function cleanup_far_pois(center_page_idx: IVec): void {
  const { poi_map, clear_pois } = usePoiStore.getState();

  for (const [page_idx] of poi_map) {
    const dist: u32 = CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx));
    if (dist > CLEANUP_RADIUS) clear_pois(page_idx);
  }
}

/** One Demand per uncached page within LOAD_RADIUS — the scheduler dedups
 * against whatever it's already carrying, so this can be rebuilt in full on
 * every effect run. */
function poi_demands(center_page_idx: IVec): Demand[] {
  const { get_pois, set_pois } = usePoiStore.getState();
  const demands: Demand[] = [];

  for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
    for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
      const delta = ivec.gather(BigInt(dx), BigInt(dy), 0n);
      // center_page_idx is already TO_XY-normalized and delta carries no Z.
      const page_idx: IVec = ivec.add(center_page_idx, delta);
      if (get_pois(page_idx) !== undefined) continue;

      demands.push({
        key: `${CATEGORY}:${page_idx}`,
        category: CATEGORY,
        priority: CHEBYSHEV_DIST(delta) * 2 + PRIORITY_BIAS,
        run: async (signal) => {
          const phases: Phases = {};
          set_pois(page_idx, await fetch_poi(page_idx, signal, phases));
          return phases;
        },
      });
    }
  }
  return demands;
}

/**
 * Demands POI for every page within LOAD_RADIUS of the view center that
 * isn't cached yet — same radius terrain/use_tile_fetcher.ts loads at, via
 * the shared fetch_scheduler.ts so the two subsystems share one concurrency
 * quota. Reuses terrain's own CLEANUP_RADIUS to evict pages once they fall
 * out of range: a single dense urban page can hold thousands of records
 * uncapped, so leaving poi_map unbounded for a whole session's panning is a
 * real memory concern.
 *
 * Gated on settings.camera's own show_poi: hidden means an empty wishlist —
 * no new fetch, and any POI request still in flight is aborted by the
 * scheduler's own yield. Cached pages in poi_map are left as-is so toggling
 * back on feels instant for whatever's still in range.
 */
export function usePoiFetcher(): void {
  const map_center = useMapStore((state) => state.map_center);
  const show_poi = useSettingsStore((state) => state.show_poi);

  useEffect(() => {
    if (map_center === null || !show_poi) {
      fetch_sync(CATEGORY, []);
      return;
    }

    // TO_XY drops Z (matches store.ts's own page keys) — L3_IDX shifts
    // X/Y/Z together, and CHEBYSHEV_DIST's max would let elevation alone
    // defeat eviction/fetch regardless of real X/Y distance.
    const center_page_idx: IVec = TO_XY(L3_IDX(map_center.i));

    cleanup_far_pois(center_page_idx);
    fetch_sync(CATEGORY, poi_demands(center_page_idx));
  }, [map_center, show_poi]);
}
