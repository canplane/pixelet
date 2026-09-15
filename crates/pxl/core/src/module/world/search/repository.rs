// pxl_core/module/world/search/repository.rs

//! Forward search's kernel half — always the live API, never a static bake:
//! free-text search has no finite page key to bake ahead of time. This is the
//! one world feature that genuinely requires a reachable server at request
//! time, not just at ingest/bake time. The request is the shell's.

use core::fmt;

use xpute_core::kit::json::{read_json, JsonKind, JsonValue};

use crate::lib::env::route::SEARCH_URL;
use crate::module::world::geocode::repository::{PartTable, Rows};

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
    /// empty for a POI.
    pub common: Rows,
    /// The broader divisions (region/county) containing this result's own
    /// point — assembled via geocode/label.rs, same as the current-address
    /// overlay. Populated for both kinds: it is what tells two same-named
    /// results apart.
    pub context: Rows,
    pub lon: f64,
    pub lat: f64,
}

/// The most a response holds: the API answers at most 50.
const RESULTS: usize = 64;
const PARTS: usize = 1024;
const NAMES: usize = 8192;

/// One response: its results, and the parts and names they hold, cleared
/// before the next.
pub struct SearchTable {
    pub parts: PartTable,
    results: Vec<SearchResult>,
}

impl SearchTable {
    pub fn new() -> SearchTable {
        SearchTable {
            parts: PartTable::new(PARTS, NAMES),
            results: Vec::with_capacity(RESULTS),
        }
    }

    pub fn clear(&mut self) {
        self.parts.clear();
        self.results.clear();
    }

    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
}

impl Default for SearchTable {
    fn default() -> Self {
        SearchTable::new()
    }
}

/// `URLSearchParams`' encoding of what a value displays as, byte by byte.
struct FormEncoded<T>(T);

struct FormWrite<'a, 'b>(&'a mut fmt::Formatter<'b>);

impl fmt::Write for FormWrite<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => self.0.write_char(b as char)?,
                b' ' => self.0.write_char('+')?,
                _ => write!(self.0, "%{b:02X}")?,
            }
        }
        Ok(())
    }
}

impl<T: fmt::Display> fmt::Display for FormEncoded<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        write!(FormWrite(f), "{}", self.0)
    }
}

/// The request's URL: the query, and the point to search near — written
/// where the URL is written.
#[derive(Clone, Copy)]
pub struct SearchUrl<'a> {
    query: &'a str,
    near: Option<(f64, f64)>,
}

pub fn search_url(query: &str, near: Option<(f64, f64)>) -> SearchUrl<'_> {
    SearchUrl { query, near }
}

impl fmt::Display for SearchUrl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{SEARCH_URL}?q={}", FormEncoded(self.query))?;
        if let Some((lon, lat)) = self.near {
            write!(f, "&near={}", FormEncoded(format_args!("{lon},{lat}")))?;
        }
        Ok(())
    }
}

/// One result as the JSON carries it; none where it does not read.
fn read_result(t: &mut PartTable, r: JsonValue) -> Option<SearchResult> {
    let (lon, lat) = (r.get("lon")?.as_f64()?, r.get("lat")?.as_f64()?);
    let kind = if r.get("kind")?.as_str()?.eq_str("division") { SearchKind::Division } else { SearchKind::Poi };
    let gers_id = r.get("gers_id")?.as_str()?.to_string();
    let name = r.get("name")?.as_str()?.to_string();
    let category = r.get("category").and_then(|c| c.as_str()).map(|c| c.to_string()).unwrap_or_default();
    Some(SearchResult {
        gers_id,
        kind,
        name,
        category,
        common: t.read_common(r.get("common")),
        context: t.read_parts(r.get("context")),
        lon,
        lat,
    })
}

/// A response body, as `res.json()` read it, into `table` cleared first;
/// false where it does not read. Results past what the table holds are left
/// out.
pub fn parse_search(table: &mut SearchTable, body: &str) -> bool {
    table.clear();
    let Some(root) = read_json(body).filter(|r| r.kind() == JsonKind::Array) else { return false };
    for r in root.items() {
        if table.results.len() == RESULTS {
            break;
        }
        let Some(result) = read_result(&mut table.parts, r) else { continue };
        table.results.push(result);
    }
    true
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod test;
