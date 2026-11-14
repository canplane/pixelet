// @pxl/client/ui/feature/search/fetch.ts

import {
  search,
  type SearchAnswer,
  type SearchPart,
} from "@/service/origin/search.ts";
import type { NamePart } from "@/ui/feature/division/label.ts";
import { DIVISION_SUBTYPE } from "@/ui/feature/division/division.spec.ts";

export interface SearchResult extends Omit<SearchAnswer, "context"> {
  context: NamePart[];
}

export async function fetch_search(
  query: string,
  opts?: Parameters<typeof search>[1],
): Promise<SearchResult[]> {
  const results = await search(query, opts);
  return results.map((r) => ({ ...r, context: r.context.flatMap(name_part) }));
}

/** A part whose subtype is newer than this client is dropped. Subtypes are
 * only ever appended, so a newer one is any number past the last this client
 * knows. */
function name_part({ subtype, approx, ...named }: SearchPart): NamePart[] {
  return subtype > DIVISION_SUBTYPE.MICROHOOD
    ? []
    : [{ ...named, subtype, area: !approx }];
}
