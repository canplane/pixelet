// pxl-geo-bake/geocode/result.rs

//! The shape reverse geocoding answers in.

use serde::Serialize;

/// Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — a fixed
/// canonical order regardless of any viewer's language, since it is baked once
/// per page and served to every viewer. Broad-to-narrow versus narrow-to-broad
/// is a language convention (Korean, Japanese and Chinese go broad-to-narrow;
/// most others reverse it, the same pattern as date formats) rather than a
/// property of the place, so joining these into one display string — and
/// picking which language's name to show per part — is the client's job against
/// whatever UI language is active.
///
/// Forward search answers in this same shape from its own crate. The shape is
/// not the Divisions theme's and does not live with it: swap the provider and
/// this stays while the subtypes change.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeocodePart {
    pub name: String,
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
