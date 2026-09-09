// @pxl/web/modules/world/poi/use_poi_fetcher.ts

import { useEffect } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { CLEANUP_RADIUS, LOAD_RADIUS } from "../terrain/config.ts";
import { fetch_poi } from "./repository.ts";
import { usePoiStore } from "./store.ts";

// Module-level, mirroring terrain/use_tile_fetcher.ts's own
// pending_page_demands: tracks pages with a fetch already in flight so a
// re-run of the effect below (e.g. a neighboring page's own fetch
// completing and changing map_center by a hair) doesn't re-request one
// that's still pending. Deliberately NOT storing an empty placeholder in
// usePoiStore while pending (the alternative) — a failed fetch would then
// have to explicitly roll that placeholder back to "not yet loaded" to be
// retried; leaving the store untouched until a real result lands means a
// failure just falls out of `pending`, and the very next effect run demands
// it again for free.
const pending: Set<IVec> = new Set();

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

/**
 * Demands POI for every page within LOAD_RADIUS of the view center that
 * isn't cached yet — same radius terrain/use_tile_fetcher.ts loads at.
 * Reuses terrain's own CLEANUP_RADIUS to evict pages once they fall out of
 * range: a single dense urban page can hold thousands of records uncapped,
 * so leaving poi_map unbounded for a whole session's panning is a real
 * memory concern.
 *
 * Gated on settings.camera's own show_poi: hidden means no fetch, no cache
 * growth, no eviction scan. Existing cached pages are left as-is so
 * toggling back on feels instant for whatever's still in range.
 */
export function usePoiFetcher(): void {
  const map_center = useMapStore((state) => state.map_center);
  const show_poi = useSettingsStore((state) => state.show_poi);

  useEffect(() => {
    if (map_center === null || !show_poi) return;

    // TO_XY drops Z (matches store.ts's own page keys) — L3_IDX shifts
    // X/Y/Z together, and CHEBYSHEV_DIST's max would let elevation alone
    // defeat eviction/fetch regardless of real X/Y distance.
    const center_page_idx: IVec = TO_XY(L3_IDX(map_center.i));

    cleanup_far_pois(center_page_idx);

    const { get_pois, set_pois } = usePoiStore.getState();

    for (let dx = -LOAD_RADIUS; dx <= LOAD_RADIUS; dx++) {
      for (let dy = -LOAD_RADIUS; dy <= LOAD_RADIUS; dy++) {
        const page_idx = ivec.add(center_page_idx, ivec.gather(BigInt(dx), BigInt(dy), 0n));
        if (get_pois(page_idx) !== undefined || pending.has(page_idx)) continue;

        pending.add(page_idx);
        fetch_poi(page_idx)
          .then((pois) => set_pois(page_idx, pois))
          .catch((err) => {
            console.error(`[poi/${ivec.repr(page_idx)}] failed to fetch:`, err);
          })
          .finally(() => pending.delete(page_idx));
      }
    }
  }, [map_center, show_poi]);
}
