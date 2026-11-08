// @pxl/client/ui/search/fetch.ts

import {
  search,
  type SearchAnswer,
  type SearchPart,
} from "@/service/origin/search.ts";
import type { NamePart } from "@/ui/division/label.ts";
import { DIVISION_SUBTYPE } from "@/ui/division/division.spec.ts";

/** One result as the search panel reads it: the search API's answer, its
 * context as the parts of a name. */
export interface SearchResult extends Omit<SearchAnswer, "context"> {
  context: NamePart[];
}

export async function fetchSearch(
  query: string,
  opts?: Parameters<typeof search>[1],
): Promise<SearchResult[]> {
  const results = await search(query, opts);
  return results.map((r) => ({ ...r, context: r.context.flatMap(namePart) }));
}

/** A part whose subtype is newer than this client is dropped. Subtypes are
 * only ever appended, so a newer one is any number past the last this client
 * knows. */
function namePart({ subtype, approx, ...named }: SearchPart): NamePart[] {
  return subtype > DIVISION_SUBTYPE.MICROHOOD
    ? []
    : [{ ...named, subtype, area: !approx }];
}
