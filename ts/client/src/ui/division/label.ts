// @pxl/client/ui/division/label.ts

import { DIVISION_SUBTYPE } from "./division.spec.ts";

/** A part of a name, a division's own: its subtype (`DIVISION_SUBTYPE`) and whether an area holds the point. */
export interface NamePart {
  name: string;
  subtype: number;
  area: boolean;
}

/** Kana, Han and Hangul: the scripts whose addresses run broad to narrow, the
 * region before the street. */
const BROAD_TO_NARROW = /[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7a3]/;

/** Overture's neighborhoods, below a locality. */
const HOODS = new Set([
  DIVISION_SUBTYPE.MACROHOOD,
  DIVISION_SUBTYPE.NEIGHBORHOOD,
  DIVISION_SUBTYPE.MICROHOOD,
]);

/** Between the address and the neighborhood. */
const TIER_SEPARATOR = " · ";

function joined(names: string[], broadFirst: boolean): string {
  return broadFirst ? names.join(" ") : [...names].reverse().join(", ");
}

/** An address, then its neighborhood apart: Overture's hoods, and any part
 * found by its point rather than held by an area, are no part of an address,
 * as Google's address descriptors give areas beside the address. A
 * neighborhood named as the address ends is left out, as Pelias's labels drop
 * a name its first admin part repeats (`dedupeNameAndFirstLabelElement`). */
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
