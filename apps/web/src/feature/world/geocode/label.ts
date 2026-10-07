// @pxl/web/feature/world/geocode/label.ts

/** `name` is the provider's own, `names` the same by language tag where it has one. */
export interface PlacePart {
  name: string;
  names?: Record<string, string> | null;
}

// Languages that write addresses broad-to-narrow ("강남구 역삼1동").
const BROAD_TO_NARROW_LANGS = new Set(["ko", "ja", "zh"]);

/** The tag a place's names are keyed by: "ko-KR" reads as "ko". */
export function primaryLanguage(tag: string): string {
  return tag.split("-")[0].toLowerCase();
}

export function localName(part: PlacePart, lang: string): string {
  return part.names?.[lang] ?? part.name;
}

export function assembleLabel(parts: PlacePart[], lang: string): string {
  const names = parts.map((p) => localName(p, lang));
  return BROAD_TO_NARROW_LANGS.has(lang) ? names.join(" ") : [...names].reverse().join(", ");
}
