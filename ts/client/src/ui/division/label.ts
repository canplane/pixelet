// @pxl/client/ui/division/label.ts

import { DIVISION_SUBTYPE } from "./division.spec.ts";

/** One division's name, a part of a place's label. `subtype` is one of
 * `DIVISION_SUBTYPE`. `area` is true when the division's area holds the
 * point, and false when the division was found by its own point instead. */
export interface NamePart {
  name: string;
  subtype: number;
  area: boolean;
}

/** Kana, Han and Hangul: the scripts whose addresses run from broad to narrow,
 * with the region before the street. */
const BROAD_TO_NARROW = /[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7a3]/;

/** The subtypes Overture gives the neighborhoods below a locality. */
const HOODS = new Set([
  DIVISION_SUBTYPE.MACROHOOD,
  DIVISION_SUBTYPE.NEIGHBORHOOD,
  DIVISION_SUBTYPE.MICROHOOD,
]);

/** What separates the address from its neighborhood. */
const TIER_SEPARATOR = " · ";

function joined(names: string[], broadFirst: boolean): string {
  return broadFirst ? names.join(" ") : [...names].reverse().join(", ");
}

/** A place's label: its address, then its neighborhood set apart. Overture's
 * neighborhoods are not part of the address, and neither is any part found by
 * its point rather than held by an area; they stand beside the address, as
 * areas do in Google's address descriptors. A neighborhood with the same name
 * as the address's narrowest part is left out, as Pelias drops a name that
 * the label's first admin part repeats (`dedupeNameAndFirstLabelElement`). */
export function assembleLabel(parts: NamePart[]): string {
  const broadFirst = BROAD_TO_NARROW.test(parts[0]?.name ?? "");
  const split = parts.findIndex((p) => !p.area || HOODS.has(p.subtype));
  const address = (split < 0 ? parts : parts.slice(0, split)).map(
    (p) => p.name,
  );
  const hood = (split < 0 ? [] : parts.slice(split)).map((p) => p.name);
  if (hood[0] !== undefined && hood[0] === address.at(-1)) hood.shift();
  const tiers = [address, hood]
    .filter((t) => t.length > 0)
    .map((t) => joined(t, broadFirst));
  return (broadFirst ? tiers : tiers.reverse()).join(TIER_SEPARATOR);
}
