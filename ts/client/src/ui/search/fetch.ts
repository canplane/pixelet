// @pxl/client/ui/search/fetch.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { get, SEARCH_URL } from "@/port/ui.ts";
import type { NamePart } from "@/ui/division/label.ts";
import { DIVISION_SUBTYPE } from "@/ui/division/division.spec.ts";
import type { SEARCH_KIND } from "./search.spec.ts";

/** A part of a result's context as the search API sends it: a division's
 * name and its subtype's number. `approx` is set when the division is the
 * one nearest the point, not one whose area holds it. */
interface WirePart {
  name: string;
  subtype: number;
  approx?: boolean;
}

/** One result as the search API answers it. Its kind and its category are
 * numbers (search.spec.ts). */
export interface SearchResult {
  gers_id: string;
  kind: (typeof SEARCH_KIND)[keyof typeof SEARCH_KIND];
  name: string;
  /** For a place, its category in place_theme.spec.ts, counted from 1, with
   * 0 for none. For a division, its subtype. */
  category: number;
  /** The divisions around the result, which tell results of the same name
   * apart. */
  context: NamePart[];
  lon: f64;
  lat: f64;
  /** The distance over the ground from `near`, in meters, as the server
   * measures it (`dist` in pxl-spec space/coord/adapter/4326.rs). It is null
   * when the query gave no `near`. */
  dist_m: f64 | null;
}

/** Search always asks the live API: free text gives no key under which
 * results could be baked ahead of time. */
export async function fetchSearch(
  query: string,
  opts?: { near?: { lon: f64; lat: f64 }; signal?: AbortSignal },
): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await get(`${SEARCH_URL}?${params.toString()}`, opts?.signal);
  if (!res.ok) {
    // The body of a refused request is never read. Cancelling it lets the
    // connection go now, rather than when the response is garbage-collected.
    await res.body?.cancel();
    throw new Error(`[search] http ${res.status}`);
  }
  const results = (await res.json()) as (Omit<SearchResult, "context"> & {
    context: WirePart[];
  })[];
  return results.map((r) => ({ ...r, context: r.context.flatMap(namePart) }));
}

/** A part whose subtype is newer than this client is dropped. Subtypes are
 * only ever appended, so a newer one is any number past the last this client
 * knows. */
function namePart({ subtype, approx, ...named }: WirePart): NamePart[] {
  return subtype > DIVISION_SUBTYPE.MICROHOOD
    ? []
    : [{ ...named, subtype, area: !approx }];
}
