// pxl-geo-spec/overture/properties.rs

//! The handful of fields either side reads out of a place's `properties`
//! JSONB. Overture's per-place shape is a rich nested object with no flat-tag
//! equivalent, so these are the paths rather than a decoded struct: nothing
//! here wants the whole document.

use crate::overture::category;

/// `properties.categories.primary` — Overture's own category, before it is
/// translated.
pub fn overture_category(properties: &serde_json::Value) -> Option<&str> {
    properties.get("categories")?.get("primary")?.as_str()
}

/// The category the client's POI theme keys off. An Overture category with no
/// entry in the table passes through unmapped; a place with no category at all
/// is a "place".
///
/// Both servers call this, and that is the reason this crate exists: the badge
/// a place draws on the map and the icon beside it in the search list come
/// from here, and they have to agree.
pub fn category_of(properties: &serde_json::Value) -> String {
    match overture_category(properties) {
        Some(primary) => category::map(primary).unwrap_or(primary).to_string(),
        None => "place".to_string(),
    }
}

/// `properties.names.primary` — the name a place is shown under, or empty when
/// Overture has none for it.
pub fn name_of(properties: &serde_json::Value) -> &str {
    properties.get("names").and_then(|n| n.get("primary")).and_then(|p| p.as_str()).unwrap_or("")
}

/// `properties.confidence` — a source-conflation quality score rather than a
/// notability one, which is the closest signal Overture carries to a rank.
pub fn confidence(properties: &serde_json::Value) -> f64 {
    properties.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0)
}
