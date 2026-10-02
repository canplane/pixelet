// pxl-geo-spec/overture/properties.rs

use crate::overture::category;

/// The path [`overture_category`] reads, for SQL that selects it.
pub const CATEGORY_SQL: &str = "properties->'categories'->>'primary'";

pub fn overture_category(properties: &serde_json::Value) -> Option<&str> {
    properties.get("categories")?.get("primary")?.as_str()
}

/// The POI badge key; the search list and the map's badge must agree on it.
pub fn category_of(properties: &serde_json::Value) -> String {
    category_named(overture_category(properties)).to_string()
}

pub fn category_named(primary: Option<&str>) -> &str {
    match primary {
        Some(primary) => category::map(primary).unwrap_or(primary),
        None => "place",
    }
}

pub fn name_of(properties: &serde_json::Value) -> &str {
    properties.get("names").and_then(|n| n.get("primary")).and_then(|p| p.as_str()).unwrap_or("")
}

/// A conflation-quality score, the closest Overture has to a rank.
pub fn confidence(properties: &serde_json::Value) -> f64 {
    properties.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0)
}
