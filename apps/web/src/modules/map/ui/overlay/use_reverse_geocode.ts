// @pxl/web/modules/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import type { IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { fetch_geocode, type GeocodePart } from "@/modules/world/geocode/repository.ts";
import { assemble_label } from "@/modules/world/geocode/label.ts";
import { fetch_sync } from "@/modules/world/sched/fetch_scheduler.ts";

// A short debounce still matters even though the real dedup key below is
// now an exact page_idx (not a rounded lat/lon string): panning fires
// map_center updates every frame, and most of those land on the same page
// as the last one — no need to even compute L3_IDX that often.
const DEBOUNCE_MS = 500;

const CATEGORY = "geocode";
// Ahead of every streaming category (terrain starts at 0, POI at 1) — this
// is a label the user is waiting to read, not background prefetch, and
// there is only ever one of it in flight.
const PRIORITY = -5;

export type ReverseGeocodeResult = {
  label: string | null;
  loading: boolean;
};

/**
 * Neighborhood label for the current map_center, baked per L3 page from
 * Overture's Divisions theme (apps/api/src/modules/tiles/geocode) — no
 * longer a live per-coordinate call to public Nominatim (see
 * discussions/20260908-overture-poi-adoption.md). Page-keyed like POI, so
 * the "have we already resolved this one" check is exact IVec equality,
 * not the old rounded-lat/lon string comparison.
 *
 * Stores the fetched result as GeocodeResult's own unjoined parts, not a
 * display string — assemble_label() runs every render against the current
 * i18n.language, so switching UI language re-orders an already-fetched
 * label instantly instead of needing a re-fetch (the data never changes,
 * only how it reads).
 */
export function useReverseGeocode(map_center: PVF3 | null): ReverseGeocodeResult {
  const { i18n } = useTranslation();
  const [parts, set_parts] = useState<GeocodePart[] | null>(null);
  const [loading, set_loading] = useState(false);

  /** The page a request has been submitted for, not the page a label was
   * received for. It exists to stop the same page being asked about twice,
   * so it has to be released again the moment an attempt stops being one —
   * see the abort handling below. */
  const submitted_page_ref = useRef<IVec | null>(null);

  useEffect(() => {
    if (map_center === null) return;

    const page_idx: IVec = L3_IDX(map_center.i);

    const timer = setTimeout(() => {
      if (page_idx === submitted_page_ref.current) return;
      submitted_page_ref.current = page_idx;

      set_loading(true);
      // A one-item wishlist: submitting a new page supersedes the previous
      // one, which fetch_scheduler.ts's own yield aborts — the exact
      // latest-wins behaviour this used to hand-roll with an AbortController,
      // now sharing one quota (and the ledger) with terrain/POI. Errors are
      // the scheduler's to log and classify.
      fetch_sync(CATEGORY, [{
        key: `${CATEGORY}:${page_idx}`,
        category: CATEGORY,
        priority: PRIORITY,
        run: async (signal) => {
          try {
            const result = await fetch_geocode(page_idx, signal);
            set_parts(result?.parts ?? null);
          } catch (e) {
            // A failed attempt is not an answer, so the page goes back to
            // being unasked. Without this the guard above treats it as
            // handled and the label never gets another chance.
            if (submitted_page_ref.current === page_idx) submitted_page_ref.current = null;
            throw e;
          } finally {
            // Aborted is the same: superseded by a *different* page has
            // already moved the ref on, so this only fires when the request
            // for this very page was cancelled — which is exactly the case
            // that has to be retryable.
            if (signal.aborted && submitted_page_ref.current === page_idx) submitted_page_ref.current = null;
            // clears on success and on a real failure alike, but not when
            // superseded — the demand that superseded this one already set
            // it back to true.
            if (!signal.aborted) set_loading(false);
          }
        },
      }]);
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [map_center]);

  // Only on unmount. Clearing the wishlist on every map_center change
  // cancelled a request that was still the right one to be making: a search
  // jump sets the centre twice — once on the jump and again when the target
  // page's tile arrives and it re-grounds — so the second one aborted the
  // first one's geocode. Submitting a new wishlist already supersedes the
  // old, which is what the comment above describes, so there was never a
  // reason to clear in between.
  useEffect(() => () => fetch_sync(CATEGORY, []), []);

  const label = parts ? assemble_label(parts, i18n.language) : null;
  return { label, loading };
}
