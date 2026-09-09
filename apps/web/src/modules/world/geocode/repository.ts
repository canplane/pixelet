// @pxl/web/modules/world/geocode/repository.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { GEOCODE_TILE_BASE_URL } from "@/lib/env/config.ts";

/** One administrative division, mirrors the API's own GeocodePart
 * (bake/geocode/provider.ts) — `name` is Overture's local-language
 * `names.primary`, `common` is a language-tag-keyed subset of
 * `names.common` present only when Overture had a translation. */
export type GeocodePart = { name: string; common?: Record<string, string> };

/** Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — see the
 * API's own provider.ts doc comment for why this stays unjoined (and each
 * part's language unresolved) until use_reverse_geocode.ts assembles it
 * against the viewer's own UI language. */
export type GeocodeResult = { parts: GeocodePart[] };

/**
 * Reverse-geocode transport — the pre-baked static set when one is
 * configured (apps/api/src/bake/geocode/tools/bake_geocode_tiles.ts), the
 * live API otherwise. Same HAS_STATIC_POI shape poi/repository.ts uses: a
 * missing object just means "outside Overture's Divisions coverage for
 * this page", not a genuine failure, so a 404 resolves straight to `null`.
 *
 * Retired a direct client-side call to public Nominatim
 * (use_reverse_geocode.ts used to build a lat/lon+zoom URL against it) —
 * see discussions/20260908-overture-poi-adoption.md. Reverse geocoding is
 * now page-keyed like everything else this app bakes, not per exact
 * coordinate.
 */
function geocode_url(key: string): string {
  return `${GEOCODE_TILE_BASE_URL}/${key}`;
}

export async function fetch_geocode(page_idx: IVec, signal?: AbortSignal): Promise<GeocodeResult | null> {
  const res = await fetch(geocode_url(page_idx.toString()), { signal });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`[geocode/${ivec.repr(page_idx)}] http ${res.status}`);
  return await res.json() as GeocodeResult;
}
