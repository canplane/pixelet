// @pxl/web/modules/map/ui/overlay/use_reverse_geocode.ts

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { GEOCODE_BASE_URL } from "@/lib/env/config.ts";

// Nominatim's usage policy caps this at 1 req/sec for the shared public
// instance — this debounce is much more conservative than that floor
// (fires only once panning/zooming has actually settled), so normal use
// never gets close to the limit regardless of how often map_center changes
// mid-gesture.
const DEBOUNCE_MS = 1500;

const G4326_SCRATCH: G4326 = [0, 0, 0];

// How many of display_name's leading (most-specific-first) comma segments
// to keep — 3 keeps roughly "neighborhood, district, city" for a typical
// zoom=14 result without also pulling in the trailing province/postcode/
// country segments display_name always ends with.
const DISPLAY_NAME_SEGMENTS = 3;

export type ReverseGeocodeResult = {
  /** A locality-level address string in whatever format Nominatim's own display_name uses for that country/language — or null while unresolved/unavailable. */
  label: string | null;
  loading: boolean;
};

/**
 * Resolves `map_center` to a locality-level address label via OpenStreetMap's
 * Nominatim reverse-geocoding service (no API key — see the fair-use
 * constraints in the debounce comment above, and the attribution requirement
 * surfaced in current_address.tsx). Localized to the app's current i18n
 * language (accept-language below) — re-fetches automatically on a language
 * change, same as any other translated string.
 *
 * Built from Nominatim's own `display_name` (trimmed to its first
 * DISPLAY_NAME_SEGMENTS comma-separated components), not a hand-picked set
 * of `address` fields (suburb/city/state/...) joined in a fixed order — that
 * approach baked in Korea's own "si gu dong" administrative hierarchy and
 * word order, which doesn't generalize: other countries' address components
 * differ (not every place has a "borough", county-vs-city varies by
 * country), and reading order isn't universal either (Korean addresses go
 * broad-to-specific; English/most Western addresses go specific-to-broad).
 * display_name already encodes both the right components AND the right
 * order for wherever `map_center` actually is, since Nominatim itself picks
 * that per-country/locale — this just borrows that instead of re-deriving
 * it.
 *
 * zoom=14 asks for neighbourhood-level resolution, not a full street
 * address (this app only wants "which neighborhood") — deliberately coarser
 * so the response is cheaper and less likely to include an exact
 * building/lot number.
 *
 * Coverage caveat: administrative-area coverage comes from OSM community
 * mapping, not an official registry — a rural/outskirts point may come back
 * with a sparser display_name, and a non-Korean language may have sparser
 * name coverage than Korean does for the same area (Nominatim falls back to
 * a romanization or the native name when no translated name tag exists).
 * `label` stays null when nothing usable comes back, rather than showing a
 * partial/misleading result.
 */
export function useReverseGeocode(map_center: PVF3 | null): ReverseGeocodeResult {
  const { i18n } = useTranslation();
  const [label, set_label] = useState<string | null>(null);
  const [loading, set_loading] = useState(false);

  // coordinates+language already resolved (or in flight) for — skips a
  // redundant network round-trip if map_center settles back to a spot
  // already looked up in the same language (e.g. nudging back and forth
  // across the same intersection), while still forcing a real re-fetch on a
  // language switch even at the exact same coordinates.
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
      const key = `${lng.toFixed(4)},${lat.toFixed(4)},${i18n.language}`;
      if (key === resolved_key_ref.current) return;
      resolved_key_ref.current = key;

      set_loading(true);

      (async () => {
        try {
          const url = `${GEOCODE_BASE_URL}/reverse?format=jsonv2&lat=${lat}&lon=${lng}&zoom=14&accept-language=${i18n.language}`;
          const res = await fetch(url, { signal: controller.signal });
          if (!res.ok) throw new Error(`reverse geocode http ${res.status}`);

          const data = await res.json();
          const display_name: string | undefined = data?.display_name;
          const segments = display_name?.split(",").map((s) => s.trim()).filter(Boolean) ?? [];
          set_label(segments.length > 0 ? segments.slice(0, DISPLAY_NAME_SEGMENTS).join(", ") : null);
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
  }, [map_center, i18n.language]);

  return { label, loading };
}
