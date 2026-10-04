// @pxl/web/feature/map/geocode/label.ts

/** `name` is the provider's own, `names` the same by language tag where it has one. */
export interface PlacePart {
  name: string;
  names?: Record<string, string>;
}

// Languages that write addresses broad-to-narrow ("강남구 역삼1동").
const BROAD_TO_NARROW_LANGS = new Set(["ko", "ja", "zh"]);

function displayPartName(part: PlacePart, lang: string): string {
  return part.names?.[lang] ?? part.name;
}

export function assembleLabel(parts: PlacePart[], language: string): string {
  const lang = language.split("-")[0].toLowerCase();
  const names = parts.map((p) => displayPartName(p, lang));
  return BROAD_TO_NARROW_LANGS.has(lang) ? names.join(" ") : [...names].reverse().join(", ");
}
