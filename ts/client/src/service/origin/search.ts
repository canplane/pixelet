// @pxl/client/service/origin/search.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { get } from "./route.ts";
import { SEARCH_URL } from "./route.spec.ts";

/** A part of a result's context as the search API sends it: a division's
 * name and its subtype's number. `approx` is set when the division is the
 * one nearest the point, not one whose area holds it. */
export interface SearchPart {
  name: string;
  subtype: number;
  approx?: boolean;
}

/** The numbers of `kind` and `category` are defined by
 * ui/feature/search/search.spec.ts. */
export interface SearchAnswer {
  gers_id: string;
  kind: number;
  name: string;
  /** For a place, its category in place_theme.spec.ts, counted from 1, with
   * 0 for none. For a division, its subtype. */
  category: number;
  /** The divisions around the result, which tell results of the same name
   * apart. */
  context: SearchPart[];
  lon: f64;
  lat: f64;
  /** The distance over the ground from `near`, in meters, as the server
   * measures it (`dist` in pxl-spec space/coord/adapter/4326.rs). It is null
   * when the query gave no `near`. */
  dist_m: f64 | null;
}

/** Search always asks the live API: free text gives no key under which
 * results could be baked ahead of time. */
export async function search(
  query: string,
  opts?: { near?: { lon: f64; lat: f64 }; signal?: AbortSignal },
): Promise<SearchAnswer[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await get(`${SEARCH_URL}?${params.toString()}`, opts?.signal);
  if (!res.ok) {
    // The body of a refused request is never read. Canceling it lets the
    // connection go now, rather than when the response is garbage-collected.
    await res.body?.cancel();
    throw new Error(`[search] http ${res.status}`);
  }
  return (await res.json()) as SearchAnswer[];
}
