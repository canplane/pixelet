// @pxl/web/feature/search/repository.ts

import type { f64 } from "@xpute/core/abi/word.ts";
import { originGet } from "@platform/origin.ts";
import { SEARCH_URL } from "@/config/route.spec.ts";
import type { DivisionPart } from "@/feature/world/geocode/label.ts";

export interface SearchResult {
  gers_id: string;
  kind: "poi" | "division";
  name: string;
  category: string;
  /** Only for `kind: 'division'`; null for POIs. */
  common: Record<string, string> | null;
  /** Tells same-named results apart; replaced an address field that could name another city. */
  context: DivisionPart[];
  lon: f64;
  lat: f64;
}

/** Always the live API: free-text search has no page key to bake ahead. */
export async function fetchSearch(query: string, opts?: { near?: { lon: f64; lat: f64 }; signal?: AbortSignal }): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await originGet(`${SEARCH_URL}?${params.toString()}`, opts?.signal);
  if (!res.ok) throw new Error(`[search] http ${res.status}`);
  return await res.json() as SearchResult[];
}
