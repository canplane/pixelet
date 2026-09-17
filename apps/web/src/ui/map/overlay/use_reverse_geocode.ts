// @pxl/web/module/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { fetch_geocode, type GeocodePart } from "@/module/world/geocode/repository.ts";
import { assemble_label } from "@/module/world/geocode/label.ts";

// A short debounce still matters even though the real dedup key below is
// now an exact page_idx (not a rounded lat/lon string): panning fires
// map_center updates every frame, and most of those land on the same page
// as the last one — no need to even compute L3_IDX that often.
const DEBOUNCE_MS = 500;

export type ReverseGeocodeResult = {
  label: string | null;
  loading: boolean;
};

/**
 * Neighborhood label for the current map_center, baked per L3 page from
 * Overture's Divisions theme (apps/api/src/module/tile/geocode) — no
 * longer a live per-coordinate call to public Nominatim (see
 * issue/20260908-overture-poi-adoption.md). Page-keyed like POI, so
 * the "have we already resolved this one" check is exact IVec equality,
 * not the old rounded-lat/lon string comparison.
 *
 * Stores the fetched result as GeocodeResult's own unjoined parts, not a
 * display string — assemble_label() runs every render against the current
 * i18n.language, so switching UI language re-orders an already-fetched
 * label instantly instead of needing a re-fetch (the data never changes,
 * only how it reads).
 */
export function useReverseGeocode(map_center: [number, number] | null): ReverseGeocodeResult {
  const { i18n } = useTranslation();
  const [parts, set_parts] = useState<GeocodePart[] | null>(null);
  const [loading, set_loading] = useState(false);

  /** The page a request has been submitted for, not the page a label was
   * received for. It exists to stop the same page being asked about twice,
   * so it has to be released again the moment an attempt stops being one —
   * see the abort handling below. */
  const submitted_page_ref = useRef<IVec | null>(null);
  /** The request in flight: one at a time, the latest page's. */
  const request_ref = useRef<AbortController | null>(null);

  // The kernel hands the centre over as degrees (the snapshot's `map.center`),
  // and the page is what the label is keyed by.
  const page_idx: IVec | null = map_center === null ? null : L3_IDX(epsg4326.project({ i: 0n, f: 0n }, map_center).i);

  useEffect(() => {
    if (page_idx === null) return;

    const timer = setTimeout(() => {
      if (page_idx === submitted_page_ref.current) return;
      submitted_page_ref.current = page_idx;

      set_loading(true);
      // Latest wins: a new page's request aborts the one before it.
      request_ref.current?.abort();
      const controller = new AbortController();
      request_ref.current = controller;
      const signal = controller.signal;
      fetch_geocode(page_idx, signal)
        .then((result) => set_parts(result?.parts ?? null))
        .catch(() => {
          // A failed attempt is not an answer, so the page goes back to
          // being unasked. Without this the guard above treats it as
          // handled and the label never gets another chance.
          if (submitted_page_ref.current === page_idx) submitted_page_ref.current = null;
        })
        .finally(() => {
          // Aborted is the same: superseded by a *different* page has
          // already moved the ref on, so this only fires when the request
          // for this very page was cancelled — which is exactly the case
          // that has to be retryable.
          if (signal.aborted && submitted_page_ref.current === page_idx) submitted_page_ref.current = null;
          // clears on success and on a real failure alike, but not when
          // superseded — the request that superseded this one already set
          // it back to true.
          if (!signal.aborted) set_loading(false);
        });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [page_idx]);

  // Only on unmount. Aborting on every page change would cancel a request
  // that was still the right one to be making: a search jump sets the centre
  // twice — once on the jump and again when the target page's tile arrives
  // and it re-grounds — so the second one aborted the first one's geocode.
  useEffect(() => () => request_ref.current?.abort(), []);

  const label = parts ? assemble_label(parts, i18n.language) : null;
  return { label, loading };
}
