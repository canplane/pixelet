// pxl-overture/division.rs

//! Overture's Divisions theme as both servers read it: the label a point
//! resolves to, and the two subtype ladders that decide what goes in it.
//!
//! The ladders are here rather than beside either caller because a search
//! result's context and the current-location label have to agree about what
//! the fine level of a place is called. They were the same two lists in two
//! files before.

use serde::Serialize;

/// Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — a fixed
/// canonical order regardless of any viewer's language. Joining these into one
/// display string, and picking which language's name to show per part, is the
/// client's job against whatever UI language is active: broad-to-narrow versus
/// narrow-to-broad is a language convention, not a property of the place.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeocodePart {
    pub name: String,
    /// A language-tag-keyed name map, when Overture has one for this division.
    /// Omitted rather than null, so the client's own optional field reads the
    /// same whichever side produced it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common: Option<serde_json::Value>,
    /// Set only on a part that came from a nearby point rather than from a
    /// polygon containing the query — the label is then an approximation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approx: Option<bool>,
}

impl GeocodePart {
    pub fn new(name: String, common: Option<serde_json::Value>) -> GeocodePart {
        GeocodePart { name, common, approx: None }
    }

    pub fn approx(mut self) -> GeocodePart {
        self.approx = Some(true);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeocodeResult {
    pub parts: Vec<GeocodePart>,
}

/// region (도/특별시) → county (시/구), the canonical broad-to-narrow storage
/// order. Sorted strictly by subtype rather than by name pattern or polygon
/// size: Korea's administrative depth is not uniform — Seoul has three real
/// levels under `region`, and in a split city like Seongnam `county` lands on
/// the city rather than on a district — so this renders whatever is actually
/// the finest broad-level data Overture has for a point rather than assuming a
/// fixed real-world meaning per subtype.
pub const BROAD_SUBTYPE_ORDER: [&str; 2] = ["region", "county"];

/// Overture's four sub-city subtypes, most-specific-for-Korea first: a formal
/// 동 (`locality`) is what most reverse-geocode consumers actually want, so it
/// wins over an informal or commercial area sharing the same point even when
/// both have a polygon there. Only the first match is used — unlike
/// region ⊃ county, Overture does not guarantee these four nest cleanly, since
/// a `microhood` can be a business district overlapping a `locality` without
/// being contained by any real hierarchy.
pub const FINE_SUBTYPE_PRIORITY: [&str; 4] = ["locality", "neighborhood", "microhood", "macrohood"];

/// The rank of a broad subtype, or none when it is not one.
pub fn broad_rank(subtype: &str) -> Option<usize> {
    BROAD_SUBTYPE_ORDER.iter().position(|s| *s == subtype)
}
