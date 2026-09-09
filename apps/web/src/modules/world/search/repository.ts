// @pxl/web/modules/world/search/repository.ts

import type { GeocodePart } from "@/modules/world/geocode/repository.ts";

/**
 * One matched place, WGS84 lon/lat — mirrors the API's own SearchResult
 * (modules/world/search/provider.ts).
 */
export type SearchResult = {
  gers_id: string;
  kind: "poi" | "division";
  name: string;
  category: string;
  /** Overture's own street-level address (locality + freeform), shown
   * exactly as given — see the API's own SearchResult doc comment on why
   * this is never reformatted. `null` when Overture had no address data
   * for this place, and always null for a "division" result. */
  address: string | null;
  /** Mirrors GeocodePart's own `common` (world/geocode/repository.ts) —
   * populated for `kind: "division"`, always `null` for `kind: "poi"`. */
  common: Record<string, string> | null;
  /** The broader divisions (region/county) containing this result's own
   * point — assemble via world/geocode/label.ts's own assemble_label(),
   * same as the current-address overlay. Always `[]` for `kind: "poi"`
   * (its own `address` already covers this). */
  context: GeocodePart[];
  lon: number;
  lat: number;
};

/**
 * Forward search's own transport — always the live API, never a static
 * bake: unlike POI/geocode/repository.ts, there is no HAS_STATIC_* branch
 * here, because free-text search has no finite L3-page key to bake ahead
 * of time (see the API's own search/service.ts top comment). This is the
 * one world/ feature that genuinely requires a reachable server at request
 * time, not just at ingest/bake time.
 */
export async function fetch_search(query: string, opts?: { near?: { lon: number; lat: number }; signal?: AbortSignal }): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await fetch(`/api/world/search?${params.toString()}`, { signal: opts?.signal });
  if (!res.ok) throw new Error(`[search] http ${res.status}`);
  return await res.json() as SearchResult[];
}
