// @pxl/web/feature/map/geocode/repository.ts

import { GEOCODE_TILE_BASE_URL } from "@/config/deploy.ts";

/** One administrative division, mirrors the API's own GeocodePart
 * (bake/geocode/provider.ts) — `name` is Overture's local-language
 * `names.primary`, `common` is a language-tag-keyed subset of
 * `names.common` present only when Overture had a translation. */
export interface GeocodePart {
  name: string;
  common?: Record<string, string>;
}

/** Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — see the
 * API's own provider.ts doc comment for why this stays unjoined (and each
 * part's language unresolved) until use-reverse-geocode.ts assembles it
 * against the viewer's own UI language. */
export interface GeocodeResult {
  parts: GeocodePart[];
}

/**
 * Reverse-geocode transport — the pre-baked static set when one is
 * configured (crates/pxl/server/geo-bake/src/geocode/tool/bake_geocode_tiles.rs), the
 * live API otherwise. Same PUBLISHED_TILES shape poi/repository.ts uses: a
 * missing object just means "outside Overture's Divisions coverage for
 * this page", not a genuine failure, so a 404 resolves straight to `null`.
 *
 * Retired a direct client-side call to public Nominatim
 * (use-reverse-geocode.ts used to build a lat/lon+zoom URL against it) —
 * see issue/20260908-overture-poi-adoption.md. Reverse geocoding is
 * now page-keyed like everything else this app bakes, not per exact
 * coordinate.
 */
function geocodeUrl(key: string): string {
  return `${GEOCODE_TILE_BASE_URL}/${key}`;
}

/** `page` is the snapshot's own `map.page` — the decimal L3 index, which is
 * both the bucket key and the last segment of the URL. */
export async function fetchGeocode(page: string, signal?: AbortSignal): Promise<GeocodeResult | null> {
  const res = await fetch(geocodeUrl(page), { signal });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`[geocode/${page}] http ${res.status}`);
  return await res.json() as GeocodeResult;
}
