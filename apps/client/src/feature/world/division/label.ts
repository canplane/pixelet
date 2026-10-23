// @pxl/client/feature/world/division/label.ts

import { DIVISION_SUBTYPE } from "./division.spec.ts";

/** `name` is the provider's own, `names` the same by language tag where it has one. */
export interface DivisionPart {
  name: string;
  names?: Record<string, string> | null;
}

/** A part of a name: its subtype (`DIVISION_SUBTYPE`) and whether an area holds the point. */
export interface NamePart extends DivisionPart {
  subtype: number;
  area: boolean;
}

// Languages that write addresses broad-to-narrow ("강남구 역삼1동").
const BROAD_TO_NARROW_LANGS = new Set(["ko", "ja", "zh"]);

/** Overture's neighborhoods, below a locality. */
const HOODS = new Set([DIVISION_SUBTYPE.MACROHOOD, DIVISION_SUBTYPE.NEIGHBORHOOD, DIVISION_SUBTYPE.MICROHOOD]);

/** Between the address and the neighborhood. */
const TIER_SEPARATOR = " · ";

/** The tag a place's names are keyed by: "ko-KR" reads as "ko". */
export function primaryLanguage(tag: string): string {
  return tag.split("-")[0].toLowerCase();
}

export function localName(part: DivisionPart, lang: string): string {
  return part.names?.[lang] ?? part.name;
}

function joined(names: string[], lang: string): string {
  return BROAD_TO_NARROW_LANGS.has(lang) ? names.join(" ") : [...names].reverse().join(", ");
}

/** An address, then its neighborhood apart: Overture's hoods, and any part
 * found by its point rather than held by an area, are no part of an address,
 * as Google's address descriptors give areas beside the address. A
 * neighborhood named as the address ends is left out, as Pelias's labels drop
 * a name its first admin part repeats (`dedupeNameAndFirstLabelElement`). */
export function assembleLabel(parts: NamePart[], lang: string): string {
  const split = parts.findIndex((p) => !p.area || HOODS.has(p.subtype));
  const address = (split < 0 ? parts : parts.slice(0, split)).map((p) => localName(p, lang));
  const hood = (split < 0 ? [] : parts.slice(split)).map((p) => localName(p, lang));
  if (hood[0] !== undefined && hood[0] === address.at(-1)) hood.shift();
  const tiers = [address, hood].filter((t) => t.length > 0).map((t) => joined(t, lang));
  return (BROAD_TO_NARROW_LANGS.has(lang) ? tiers : tiers.reverse()).join(TIER_SEPARATOR);
}
