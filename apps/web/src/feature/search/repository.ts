// @pxl/web/feature/search/repository.ts

import type { f64 } from "@xpute/core/abi/word.ts";
import { origin_get } from "@platform/origin.ts";
import { SEARCH_URL } from "@/config/route.ts";
import type { PlacePart } from "@/feature/map/geocode/label.ts";

/**
 * One matched place, WGS84 lon/lat — mirrors the API's own SearchResult
 * (crates/pxl/server/geo/src/search/overture.rs).
 */
export interface SearchResult {
  gers_id: string;
  kind: "poi" | "division";
  name: string;
  category: string;
  /** The result's own name by language tag, the same shape a PlacePart's
   * `names` is — populated for `kind: "division"`, always `null` for
   * `kind: "poi"`. */
  common: Record<string, string> | null;
  /** The broader divisions (region/county) containing this result's own
   * point — assemble via map/geocode/label.ts's own assembleLabel(), same
   * as the current-address overlay. Populated for both kinds: it is what
   * tells two same-named results apart, and it replaced a raw address field
   * that could name a different city than its own coordinates. */
  context: PlacePart[];
  lon: f64;
  lat: f64;
}

/**
 * Forward search's own transport — always the live API, never a static
 * bake: unlike the POI and geocode pages, there is no PUBLISHED_TILES branch
 * here, because free-text search has no finite L3-page key to bake ahead
 * of time (see pxl-geo search/endpoint.rs's top comment). This is the
 * one world/ feature that genuinely requires a reachable server at request
 * time, not just at ingest/bake time.
 */
export async function fetchSearch(query: string, opts?: { near?: { lon: f64; lat: f64 }; signal?: AbortSignal }): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await origin_get(`${SEARCH_URL}?${params.toString()}`, opts?.signal);
  if (!res.ok) throw new Error(`[search] http ${res.status}`);
  return await res.json() as SearchResult[];
}
