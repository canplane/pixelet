// @pxl/web/catalog/geocode/label.ts
//
// Shared with search_modal.tsx: a GeocodePart[] chain (this app's own
// region/county/… division data) needs the same "pick a language, then
// order broad-to-narrow or narrow-to-broad by convention" treatment
// whether it came from reverse geocoding a page or from a search result's
// own `context` — factored out of use_reverse_geocode.ts so both share one
// answer instead of two copies drifting apart.

import type { GeocodePart } from "./repository.ts";

// Broad-to-narrow ("강남구 역삼1동") is a Korean/Japanese/Chinese address
// convention, not a property of the place itself — the same pattern shows
// up in how these languages write dates (year first). Every other
// supported language reverses it ("Yeoksam 1-dong, Gangnam-gu"). Extend
// this list, not the server, if a future UI language needs the same
// convention — a GeocodePart[] chain itself never changes.
const BROAD_TO_NARROW_LANGS = new Set(["ko", "ja", "zh"]);

// Overture's own names.primary is always the local-language (Korean) form
// — for a `lang` UI can't find in `common` (ingest_divisions.ts's own
// SUPPORTED_LANGS filter decides what's even possible here), falling back
// to `name` means "show the Korean name" rather than an empty part.
export function display_part_name(part: GeocodePart, lang: string): string {
  return part.common?.[lang] ?? part.name;
}

/** Joins a broad-to-narrow GeocodePart[] chain into one display string
 * against `language` (typically i18n.language) — the join order and
 * separator flip per BROAD_TO_NARROW_LANGS, the per-part name resolves
 * against `language` regardless. */
export function assemble_label(parts: GeocodePart[], language: string): string {
  const lang = language.split("-")[0].toLowerCase();
  const names = parts.map((p) => display_part_name(p, lang));
  return BROAD_TO_NARROW_LANGS.has(lang) ? names.join(" ") : [...names].reverse().join(", ");
}
