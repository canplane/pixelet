// @pxl/web/modules/world/poi/use_poi_fetcher.ts

import { useEffect } from "react";

import { type IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import type { Demand, Phases } from "@/modules/world/sched/demand.ts";
import { fetch_running, fetch_sync } from "@/modules/world/sched/fetch_scheduler.ts";
import { useRangeAnchor } from "@/modules/world/frame/use_range_anchor.ts";
import { FETCH_HYSTERESIS, page_priority, pages_to_hold } from "@/render/chunk_range.ts";
import { KEEP_DIST, RESIDENT_PAGES } from "../terrain/config.ts";
import { fetch_poi } from "./repository.ts";
import { poi_pages, usePoiStore } from "./store.ts";

const CATEGORY = "poi";

// POI sits one behind terrain at equal page distance — poi_layer.tsx's
// poi_anchor grounds each badge against that page's terrain, so fetching POI
// ahead of the ground it lands on just makes it wait.
const PRIORITY_BIAS = 1;

/** Evicts cached pages outside the held set, mirroring
 * terrain/use_tile_fetcher.ts's own cleanup_far_tiles — POI has no
 * pinned-page exemption to carry over (that exists there only for the
 * device marker's own grounding, not a concern here). */
function cleanup_far_pois(anchor: IVec): void {
  const { clear_pois } = usePoiStore.getState();

  const held = new Set(pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES).map(({ page_idx }) => page_idx));
  // Collected first: clearing moves the table's last page into the hole.
  const evict = Array.from(poi_pages.keys()).filter((page_idx) => !held.has(page_idx));
  for (const page_idx of evict) clear_pois(page_idx);
}

/** One Demand per uncached page within the fetch range — the scheduler dedups
 * against whatever it's already carrying, so this can be rebuilt in full on
 * every effect run. */
function poi_demands(anchor: IVec): Demand[] {
  const { get_pois, set_pois } = usePoiStore.getState();
  const demands: Demand[] = [];
  const add = (page_idx: IVec, dist2: number): void => {
    if (get_pois(page_idx) !== undefined) return;
    demands.push({
      key: `${CATEGORY}:${page_idx}`,
      category: CATEGORY,
      priority: page_priority(dist2) + PRIORITY_BIAS,
      run: async (signal) => {
        const phases: Phases = {};
        set_pois(page_idx, await fetch_poi(page_idx, signal, phases));
        return phases;
      },
    });
  };

  const held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES);
  for (const { page_idx, dist2 } of held) add(page_idx, dist2);
  // Same in-flight hysteresis as terrain — see FETCH_HYSTERESIS.
  const held_keys = new Set(held.map(({ page_idx }) => page_idx));
  for (const { page_idx, dist2 } of pages_to_hold(anchor, KEEP_DIST + FETCH_HYSTERESIS, RESIDENT_PAGES + 16)) {
    if (!held_keys.has(page_idx) && fetch_running(`${CATEGORY}:${page_idx}`)) add(page_idx, dist2);
  }
  return demands;
}

/**
 * Demands POI for every page within the fetch range that isn't cached yet —
 * the same range terrain/use_tile_fetcher.ts loads at, derived from one
 * render distance, via the shared fetch_scheduler.ts so the two subsystems
 * share one concurrency quota. Evicts on terrain's own held set once pages
 * fall out of it: a single dense urban page can hold thousands of records
 * uncapped, so leaving poi_pages unbounded for a whole session's panning is a
 * real memory concern.
 *
 * Gated on settings.camera's own show_poi: hidden means an empty wishlist —
 * no new fetch, and any POI request still in flight is aborted by the
 * scheduler's own yield. Cached pages in poi_pages are left as-is so toggling
 * back on feels instant for whatever's still in range.
 */
export function usePoiFetcher(): void {
  const anchor = useRangeAnchor();
  const show_poi = useSettingsStore((state) => state.show_poi);

  useEffect(() => {
    if (anchor === null || !show_poi) {
      fetch_sync(CATEGORY, []);
      return;
    }

    cleanup_far_pois(anchor);
    fetch_sync(CATEGORY, poi_demands(anchor));
  }, [anchor, show_poi]);
}
