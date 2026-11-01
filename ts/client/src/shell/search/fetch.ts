// @pxl/client/shell/search/fetch.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { SEARCH_URL } from "@/port/shell.ts";
import type { Name, NamePart } from "@/shell/division/label.ts";
import { DIVISION_SUBTYPE } from "@/shell/division/division.spec.ts";
import { bracket } from "@/bracket.tsx";
import type { SEARCH_KIND } from "./search.spec.ts";

/** A part as the API writes it: its subtype's number, and `approx` where no area holds the point. */
interface WirePart extends Name {
  subtype: number;
  approx?: boolean;
}

/** What the API answers, its kind and category by number (search.spec.ts). */
export interface SearchResult {
  gers_id: string;
  kind: (typeof SEARCH_KIND)[keyof typeof SEARCH_KIND];
  name: string;
  /** A place's category (place_theme.spec.ts, from 1, 0 for none), a division's subtype. */
  category: number;
  /** Only for a division; null for places. */
  common: Record<string, string> | null;
  /** Tells same-named results apart; replaced an address field that could name another city. */
  context: NamePart[];
  lon: f64;
  lat: f64;
  /** The ground meters from `near`, the server's (pxl-spec space/coord/adapter/4326.rs `dist`); null without one. */
  dist_m: f64 | null;
}

/** Always the live API: free-text search has no page key to bake ahead. */
export async function fetchSearch(
  query: string,
  opts?: { near?: { lon: f64; lat: f64 }; signal?: AbortSignal },
): Promise<SearchResult[]> {
  const params = new URLSearchParams({ q: query });
  if (opts?.near) params.set("near", `${opts.near.lon},${opts.near.lat}`);

  const res = await bracket.origin.get(
    `${SEARCH_URL}?${params.toString()}`,
    opts?.signal,
  );
  if (!res.ok) {
    // A refusal's body is not read: let go, not held until collected.
    await res.body?.cancel();
    throw new Error(`[search] http ${res.status}`);
  }
  const results = (await res.json()) as (Omit<SearchResult, "context"> & {
    context: WirePart[];
  })[];
  return results.map((r) => ({ ...r, context: r.context.flatMap(namePart) }));
}

/** None for a subtype newer than this client: subtypes are appended, so one
 * past the last it knows. */
function namePart({ subtype, approx, ...named }: WirePart): NamePart[] {
  return subtype > DIVISION_SUBTYPE.MICROHOOD
    ? []
    : [{ ...named, subtype, area: !approx }];
}
