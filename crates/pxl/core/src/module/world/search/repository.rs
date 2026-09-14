// pxl_core/module/world/search/repository.rs

//! Forward search's kernel half — always the live API, never a static bake:
//! free-text search has no finite page key to bake ahead of time. This is the
//! one world feature that genuinely requires a reachable server at request
//! time, not just at ingest/bake time. The request is the shell's.

use crate::json::{parse, Json};
use crate::lib::env::route::SEARCH_URL;
use crate::module::world::geocode::repository::{geocode_part_of, GeocodePart};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchKind {
    Poi,
    Division,
}

/// One matched place, WGS84 lon/lat — mirrors the API's own SearchResult.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub gers_id: String,
    pub kind: SearchKind,
    pub name: String,
    pub category: String,
    /// Mirrors GeocodePart's own `common` — populated for a division, always
    /// none for a POI.
    pub common: Option<Vec<(String, String)>>,
    /// The broader divisions (region/county) containing this result's own
    /// point — assembled via geocode/label.rs, same as the current-address
    /// overlay. Populated for both kinds: it is what tells two same-named
    /// results apart.
    pub context: Vec<GeocodePart>,
    pub lon: f64,
    pub lat: f64,
}

/// `URLSearchParams`' encoding of one value.
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The request's URL: the query, and the point to search near.
pub fn search_url(query: &str, near: Option<(f64, f64)>) -> String {
    let mut url = format!("{SEARCH_URL}?q={}", form_encode(query));
    if let Some((lon, lat)) = near {
        url.push_str(&format!("&near={}", form_encode(&format!("{lon},{lat}"))));
    }
    url
}

/// A response body, as `res.json()` read it.
pub fn parse_search(body: &str) -> Option<Vec<SearchResult>> {
    let root = parse(body)?;
    Some(
        root.as_array()?
            .iter()
            .filter_map(|r| {
                Some(SearchResult {
                    gers_id: r.get("gers_id")?.as_str()?.to_string(),
                    kind: if r.get("kind")?.as_str()? == "division" { SearchKind::Division } else { SearchKind::Poi },
                    name: r.get("name")?.as_str()?.to_string(),
                    category: r.get("category").and_then(Json::as_str).unwrap_or("").to_string(),
                    common: r
                        .get("common")
                        .and_then(Json::as_object)
                        .map(|fields| fields.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()),
                    context: r.get("context").and_then(Json::as_array).map(|parts| parts.iter().filter_map(geocode_part_of).collect()).unwrap_or_default(),
                    lon: r.get("lon")?.as_f64()?,
                    lat: r.get("lat")?.as_f64()?,
                })
            })
            .collect(),
    )
}
