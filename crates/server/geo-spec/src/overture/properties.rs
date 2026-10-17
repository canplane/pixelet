// pxl-geo-spec/overture/properties.rs

/// The column [`category_of`] reads, for SQL that selects it.
pub const CATEGORY_SQL: &str = "properties->>'basic_category'";

/// A place's kind as Overture names it (`basic_category`), "" for none: what
/// the places tile carries and the search list and the map look up alike.
pub fn category_of(properties: &serde_json::Value) -> String {
    properties.get("basic_category").and_then(|c| c.as_str()).unwrap_or("").to_string()
}
