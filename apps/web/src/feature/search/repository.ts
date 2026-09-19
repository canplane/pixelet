// @pxl/web/feature/search/repository.ts

import { SEARCH_URL } from "@/config/route.ts";
import type { GeocodePart } from "@/feature/map/geocode/repository.ts";

/**
 * One matched place, WGS84 lon/lat — mirrors the API's own SearchResult
 * (crates/pxl/server/geo/src/search/overture.rs).
 */
export type SearchResult = {
  gers_id: string;
  kind: "poi" | "division";
  name: string;
  category: string;
  /** Mirrors GeocodePart's own `common` (world/geocode/repository.ts) —
   * populated for `kind: "division"`, always `null` for `kind: "poi"`. */
  common: Record<string, string> | null;
  /** The broader divisions (region/county) containing this result's own
   * point — assemble via world/geocode/label.ts's own assemble_label(),
   * same as the current-address overlay. Populated for both kinds: it is
   * what tells two same-named results apart, and it replaced a raw address
   * field that could name a different city than its own coordinates. */
  context: GeocodePart[];
  lon: number;
  lat: number;
};

/**
 * Forward search's own transport — always the live API, never a static
 * bake: unlike POI/geocode/repository.ts, there is no PUBLISHED_TILES branch
 * here, because free-text search has no finite L3-page key to bake ahead
 * of time (see the API's own search/service.ts top comment). This is the
 * one world/ feature that genuinely requires a reachable server at request
 * time, not just at ingest/bake time.
 */
export async function fetch_search(query: string, opts?: { near?: { lon: number; lat: number }; signal?: AbortSignal }): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await fetch(`${SEARCH_URL}?${params.toString()}`, { signal: opts?.signal });
  if (!res.ok) throw new Error(`[search] http ${res.status}`);
  return await res.json() as SearchResult[];
}
