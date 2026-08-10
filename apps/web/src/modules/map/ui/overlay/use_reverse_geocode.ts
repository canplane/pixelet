// @pxl/web/modules/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";

import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

// Nominatim's usage policy caps this at 1 req/sec for the shared public
// instance — this debounce is much more conservative than that floor
// (fires only once panning/zooming has actually settled), so normal use
// never gets close to the limit regardless of how often map_center changes
// mid-gesture.
const DEBOUNCE_MS = 1500;

const G4326_SCRATCH: G4326 = [0, 0, 0];

export type ReverseGeocodeResult = {
  /** "OO시 OO구 OO동" style label, or null while unresolved/unavailable. */
  label: string | null;
  loading: boolean;
};

/**
 * Resolves `map_center` to a Korean 동-level label via OpenStreetMap's
 * Nominatim reverse-geocoding service (no API key — see the fair-use
 * constraints in the debounce comment above, and the attribution
 * requirement surfaced in current_address.tsx).
 *
 * zoom=14 asks for neighbourhood-level resolution, not a full street
 * address (this app only wants "which dong") — deliberately coarser so the
 * response is cheaper and less likely to include an exact building/lot number.
 *
 * Coverage caveat: Korean administrative-area coverage comes from OSM
 * community mapping, not an official registry — a rural/outskirts point may
 * come back with no suburb/village field. `label` stays null in that case
 * rather than showing a partial/misleading result.
 */
export function useReverseGeocode(map_center: PVF3 | null): ReverseGeocodeResult {
  const [label, set_label] = useState<string | null>(null);
  const [loading, set_loading] = useState(false);

  // coordinates already resolved (or in flight) for — skips a redundant
  // network round-trip if map_center settles back to a spot already looked
  // up (e.g. nudging back and forth across the same intersection).
  const resolved_key_ref = useRef<string | null>(null);

  useEffect(() => {
    if (map_center === null) return;

    // scoped to the whole effect (not just the setTimeout callback) so
    // cleanup can abort an in-flight fetch too, not just a pending timer —
    // otherwise a newer map_center couldn't cancel an older request still
    // on the wire, and responses could resolve out of order
    const controller = new AbortController();

    const timer = setTimeout(() => {
      const [lng, lat] = epsg4326.unproject(G4326_SCRATCH, map_center.i, map_center.f);
      // ~11m precision at the equator — plenty for "did the neighborhood
      // actually change", not meant as a positional record.
      const key = `${lng.toFixed(4)},${lat.toFixed(4)}`;
      if (key === resolved_key_ref.current) return;
      resolved_key_ref.current = key;

      set_loading(true);

      (async () => {
        try {
          const url = `https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat=${lat}&lon=${lng}&zoom=14&accept-language=ko`;
          const res = await fetch(url, { signal: controller.signal });
          if (!res.ok) throw new Error(`reverse geocode http ${res.status}`);

          const data = await res.json();
          const address = data?.address ?? {};
          // which field carries the 동 name varies by area in Nominatim's
          // Korean output — try the common candidates in order.
          const dong: string | undefined = address.suburb ?? address.village ?? address.quarter ?? address.neighbourhood;
          const gu: string | undefined = address.borough ?? address.city_district ?? address.county;
          const si: string | undefined = address.city ?? address.state;

          const parts = [si, gu, dong].filter((part): part is string => !!part);
          set_label(parts.length > 0 ? parts.join(" ") : null);
        } catch (err) {
          if ((err as Error)?.name !== "AbortError") {
            console.error("[reverse_geocode] failed:", err);
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

  return { label, loading };
}
