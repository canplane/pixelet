// @pxl/web/modules/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import type { IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { fetch_geocode, type GeocodePart } from "@/modules/world/geocode/repository.ts";
import { assemble_label } from "@/modules/world/geocode/label.ts";

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

  const resolved_page_ref = useRef<IVec | null>(null);

  useEffect(() => {
    if (map_center === null) return;

    const controller = new AbortController();
    const page_idx: IVec = L3_IDX(map_center.i);

    const timer = setTimeout(() => {
      if (page_idx === resolved_page_ref.current) return;
      resolved_page_ref.current = page_idx;

      set_loading(true);

      (async () => {
        try {
          const result = await fetch_geocode(page_idx, controller.signal);
          if (!controller.signal.aborted) set_parts(result?.parts ?? null);
        } catch (err) {
          if ((err as Error)?.name !== "AbortError") {
            console.error(`[reverse_geocode/${page_idx}] failed:`, err);
          }
        } finally {
          if (!controller.signal.aborted) set_loading(false);
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [map_center]);

  const label = parts ? assemble_label(parts, i18n.language) : null;
  return { label, loading };
}
