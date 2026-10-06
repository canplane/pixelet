// pxl-geo-spec/overture/properties.rs

use crate::overture::category;

/// The column [`category_named`] reads, for SQL that selects it.
pub const CATEGORY_SQL: &str = "properties->'taxonomy'";

/// The POI badge key; the search list and the map's badge must agree on it.
pub fn category_of(properties: &serde_json::Value) -> String {
    category_named(properties.get("taxonomy")).to_string()
}

/// The most specific of the taxonomy's primary and then its hierarchy, read
/// back from the primary, that a badge maps; the primary itself where none
/// does. `alternates` are other axes, with no order to choose among them.
pub fn category_named(taxonomy: Option<&serde_json::Value>) -> &str {
    let Some(primary) = taxonomy.and_then(|t| t.get("primary")).and_then(|p| p.as_str()) else {
        return "place";
    };
    let up = taxonomy
        .and_then(|t| t.get("hierarchy"))
        .and_then(|h| h.as_array())
        .into_iter()
        .flatten()
        .rev()
        .filter_map(|c| c.as_str());
    std::iter::once(primary).chain(up).find_map(category::map).unwrap_or(primary)
}

pub fn name_of(properties: &serde_json::Value) -> &str {
    properties.get("names").and_then(|n| n.get("primary")).and_then(|p| p.as_str()).unwrap_or("")
}
