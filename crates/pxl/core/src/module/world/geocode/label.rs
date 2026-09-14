// pxl_core/module/world/geocode/label.rs
//
// Shared with search: a GeocodePart chain needs the same "pick a language,
// then order broad-to-narrow or narrow-to-broad by convention" treatment
// whether it came from reverse geocoding a page or from a search result's own
// `context`, so both share one answer instead of two copies drifting apart.

use super::repository::GeocodePart;

// Broad-to-narrow ("강남구 역삼1동") is a Korean/Japanese/Chinese address
// convention, not a property of the place itself — the same pattern shows up
// in how these languages write dates (year first). Every other supported
// language reverses it ("Yeoksam 1-dong, Gangnam-gu"). Extend this list, not
// the server, if a future UI language needs the same convention.
const BROAD_TO_NARROW_LANGS: [&str; 3] = ["ko", "ja", "zh"];

// Overture's own names.primary is always the local-language (Korean) form —
// for a `lang` UI can't find in `common`, falling back to `name` means "show
// the Korean name" rather than an empty part.
pub fn display_part_name<'a>(part: &'a GeocodePart, lang: &str) -> &'a str {
    part.common.as_ref().and_then(|c| c.iter().find(|(k, _)| k == lang)).map_or(part.name.as_str(), |(_, v)| v.as_str())
}

/// Joins a broad-to-narrow GeocodePart chain into one display string against
/// `language` (typically the UI language) — the join order and separator flip
/// per BROAD_TO_NARROW_LANGS, the per-part name resolves against `language`
/// regardless.
pub fn assemble_label(parts: &[GeocodePart], language: &str) -> String {
    let lang = language.split('-').next().unwrap_or("").to_lowercase();
    let names: Vec<&str> = parts.iter().map(|p| display_part_name(p, &lang)).collect();
    if BROAD_TO_NARROW_LANGS.contains(&lang.as_str()) {
        names.join(" ")
    } else {
        names.into_iter().rev().collect::<Vec<_>>().join(", ")
    }
}
