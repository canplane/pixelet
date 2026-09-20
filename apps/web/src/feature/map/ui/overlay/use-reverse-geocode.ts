// @pxl/web/feature/map/ui/overlay/use-reverse-geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { fetchGeocode, type GeocodePart } from "@/feature/map/geocode/repository.ts";
import { assembleLabel } from "@/feature/map/geocode/label.ts";

// A short debounce still matters even though the dedup key below is an exact
// page: panning fires a snapshot every frame, and most of those land on the
// page the last one did.
const DEBOUNCE_MS = 500;

export interface ReverseGeocodeResult {
  label: string | null;
  loading: boolean;
}

/**
 * Neighborhood label for the page the map centre is on, baked per L3 page
 * from Overture's Divisions theme (crates/pxl/server/geo-bake/src/geocode/) — no
 * longer a live per-coordinate call to public Nominatim (see
 * issue/20260908-overture-poi-adoption.md). Page-keyed like POI, so
 * the "have we already resolved this one" check is exact page equality, not
 * the old rounded-lat/lon string comparison.
 *
 * The page is the kernel's (`map.page`): it indexes the centre it already
 * holds. The shell used to project the snapshot's degrees back and index them
 * itself, which is the space implemented a second time in TypeScript for one
 * call.
 *
 * Stores the fetched result as GeocodeResult's own unjoined parts, not a
 * display string — assembleLabel() runs every render against the current
 * i18n.language, so switching UI language re-orders an already-fetched
 * label instantly instead of needing a re-fetch (the data never changes,
 * only how it reads).
 */
export function useReverseGeocode(page_idx: string | null): ReverseGeocodeResult {
  const { i18n } = useTranslation();
  const [parts, setParts] = useState<GeocodePart[] | null>(null);
  const [loading, setLoading] = useState(false);

  /** The page a request has been submitted for, not the page a label was
   * received for. It exists to stop the same page being asked about twice,
   * so it has to be released again the moment an attempt stops being one —
   * see the abort handling below. */
  const submittedPageRef = useRef<string | null>(null);
  /** The request in flight: one at a time, the latest page's. */
  const requestRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (page_idx === null) return;

    const timer = setTimeout(() => {
      if (page_idx === submittedPageRef.current) return;
      submittedPageRef.current = page_idx;

      setLoading(true);
      // Latest wins: a new page's request aborts the one before it.
      requestRef.current?.abort();
      const controller = new AbortController();
      requestRef.current = controller;
      const signal = controller.signal;
      fetchGeocode(page_idx, signal)
        .then((result) => setParts(result?.parts ?? null))
        .catch(() => {
          // A failed attempt is not an answer, so the page goes back to
          // being unasked. Without this the guard above treats it as
          // handled and the label never gets another chance.
          if (submittedPageRef.current === page_idx) submittedPageRef.current = null;
        })
        .finally(() => {
          // Aborted is the same: superseded by a *different* page has
          // already moved the ref on, so this only fires when the request
          // for this very page was cancelled — which is exactly the case
          // that has to be retryable.
          if (signal.aborted && submittedPageRef.current === page_idx) submittedPageRef.current = null;
          // clears on success and on a real failure alike, but not when
          // superseded — the request that superseded this one already set
          // it back to true.
          if (!signal.aborted) setLoading(false);
        });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timer);
  }, [page_idx]);

  // Only on unmount. Aborting on every page change would cancel a request
  // that was still the right one to be making: a search jump sets the centre
  // twice — once on the jump and again when the target page's tile arrives
  // and it re-grounds — so the second one aborted the first one's geocode.
  useEffect(() => () => requestRef.current?.abort(), []);

  const label = parts ? assembleLabel(parts, i18n.language) : null;
  return { label, loading };
}
