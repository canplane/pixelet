// @pxl/web/modules/world/poi/use_poi_fetcher.ts

import { useEffect } from "react";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { LOAD_RADIUS } from "../terrain/config.ts";
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

/**
 * Demands POI for every page within LOAD_RADIUS of the view center that
 * isn't cached yet — same radius terrain/use_tile_fetcher.ts loads at, for
 * simplicity (one "how far around the camera do we care" constant, not two
 * independently tuned ones), not because POI's own ideal radius has been
 * measured separately.
 *
 * Deliberately missing, on purpose, for this first pass: terrain's own
 * CLEANUP_RADIUS eviction and device-pinned-page handling. POI payloads are
 * tiny JSON (a handful of records per page), so an unbounded, never-evicted
 * poi_map isn't a real memory concern at the scale a single session's
 * panning reaches — worth revisiting only if that stops being true.
 */
export function usePoiFetcher(): void {
  const map_center = useMapStore((state) => state.map_center);

  useEffect(() => {
    if (map_center === null) return;

    const center_page_idx: IVec = L3_IDX(map_center.i);
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
  }, [map_center]);
}
