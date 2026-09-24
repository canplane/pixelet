// @pxl/web/feature/map/geocode/label.ts
//
// One place's name assembled into a line. Shared by the address pill, which
// reads the place the map centre is in out of the kernel snapshot, and by
// search/modal.tsx, which reads each result's own context off the search
// API — the same shape from both, so the two cannot read it differently.

/** One part of a place's name, as the kernel's snapshot and the search API
 * both give it (pxl-spec place.rs): `name` is the provider's own, in
 * whatever language its rows are in, and `names` the same name by language
 * tag wherever the provider had one. */
export interface PlacePart {
  name: string;
  names?: Record<string, string>;
}

// Broad-to-narrow ("강남구 역삼1동") is a Korean/Japanese/Chinese address
// convention, not a property of the place itself — the same pattern shows
// up in how these languages write dates (year first). Every other
// supported language reverses it ("Yeoksam 1-dong, Gangnam-gu"). Extend
// this list, not the kernel, if a future UI language needs the same
// convention — the parts themselves never change.
const BROAD_TO_NARROW_LANGS = new Set(["ko", "ja", "zh"]);

// The provider's own name is always some real language's — Overture's is
// the local one — so falling back to it means "show the Korean name"
// rather than an empty part.
export function displayPartName(part: PlacePart, lang: string): string {
  return part.names?.[lang] ?? part.name;
}

/** Joins a broad-to-narrow chain into one display string against `language`
 * (typically i18n.language) — the join order and separator flip per
 * BROAD_TO_NARROW_LANGS, the per-part name resolves against `language`
 * regardless. */
export function assembleLabel(parts: PlacePart[], language: string): string {
  const lang = language.split("-")[0].toLowerCase();
  const names = parts.map((p) => displayPartName(p, lang));
  return BROAD_TO_NARROW_LANGS.has(lang) ? names.join(" ") : [...names].reverse().join(", ");
}
