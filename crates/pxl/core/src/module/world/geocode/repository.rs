// pxl_core/module/world/geocode/repository.rs

//! Reverse-geocode transport's kernel half — the pre-baked static set when
//! one is configured, the live API otherwise. A missing object just means
//! "outside Overture's Divisions coverage for this page", not a genuine
//! failure, so a 404 resolves straight to none. Reverse geocoding is
//! page-keyed like everything else this app bakes, not per exact coordinate.

use crate::json::{parse, Json};
use crate::lib::env::config::GEOCODE_TILE_BASE_URL;

/// One administrative division, mirroring the API's own GeocodePart — `name`
/// is Overture's local-language `names.primary`, `common` is a
/// language-tag-keyed subset of `names.common` present only when Overture had
/// a translation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeocodePart {
    pub name: String,
    pub common: Option<Vec<(String, String)>>,
}

/// Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — unjoined (and
/// each part's language unresolved) until label.rs assembles it against the
/// viewer's own UI language.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeocodeResult {
    pub parts: Vec<GeocodePart>,
}

pub fn geocode_url(key: &str) -> String {
    format!("{}/{key}", GEOCODE_TILE_BASE_URL())
}

/// A part as the JSON carries it.
pub fn geocode_part_of(value: &Json) -> Option<GeocodePart> {
    Some(GeocodePart {
        name: value.get("name")?.as_str()?.to_string(),
        common: value
            .get("common")
            .and_then(Json::as_object)
            .map(|fields| fields.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()),
    })
}

/// A response body, as `res.json()` read it.
pub fn parse_geocode(body: &str) -> Option<GeocodeResult> {
    let root = parse(body)?;
    Some(GeocodeResult {
        parts: root.get("parts")?.as_array()?.iter().filter_map(geocode_part_of).collect(),
    })
}
