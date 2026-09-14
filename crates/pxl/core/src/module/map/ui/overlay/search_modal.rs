// pxl_core/module/map/ui/overlay/search_modal.rs

//! Forward search, everything but the sheet it is shown in: the typed query
//! debounced, one request in flight, the results read, and each row's name,
//! where it is and how far away, as the UI renders them. Always the live API
//! (search/repository.rs); the request is the shell's.

use pxl_shared::spec::space::adapter::epsg4326;

use crate::global::{now, Global};
use crate::module::map::store::use_map_store;
use crate::module::world::change::request_world_tick;
use crate::module::world::geocode::label::assemble_label;
use crate::module::world::poi::poi_theme::resolve_poi_theme;
use crate::module::world::sched::demand::Demand;
use crate::module::world::sched::fetch_scheduler::fetch_sync;
use crate::module::world::search::repository::{parse_search, search_url, SearchKind, SearchResult};

// Debounced like use_reverse_geocode.rs's own DEBOUNCE_MS, but shorter: that
// one waits for panning to settle on a new page; this waits for typing to
// pause, a much shorter rhythm for a search box.
const DEBOUNCE_MS: f64 = 300.0;

pub const CATEGORY: &str = "search";
// The most latency-sensitive fetch in the app — a typed query with the user
// staring at an empty result list — so it outranks even geocode (-5) and
// every streaming category. Single-slot, so it never occupies more than one of
// the fetch scheduler's quota slots.
const PRIORITY: f64 = -10.0;

const EARTH_RADIUS_M: f64 = 6_378_137.0;

struct Search {
    query: String,
    typed_at: f64,
    /// The query a request was submitted for.
    submitted: Option<String>,
    /// Where the request ranks from, frozen when it was submitted: the same
    /// point the API ranked against, so the distances shown always agree with
    /// the order they are shown in, and a camera still drifting behind the open
    /// sheet cannot renumber a list being read.
    near: Option<(f64, f64)>,
    origin: Option<(f64, f64)>,
    loading: bool,
    results: Vec<SearchResult>,
}

static SEARCH: Global<Search> = Global::new();

fn search() -> &'static mut Search {
    SEARCH.get(|| Search {
        query: String::new(),
        typed_at: 0.0,
        submitted: None,
        near: None,
        origin: None,
        loading: false,
        results: Vec::new(),
    })
}

/// The query as typed. An empty one drops whatever the last keystroke left in
/// flight.
pub fn search_query(query: &str) {
    let s = search();
    s.query = query.to_string();
    s.typed_at = now();
    if query.trim().is_empty() {
        s.results.clear();
        s.loading = false;
        s.submitted = None;
        fetch_sync(CATEGORY, &[]);
    } else {
        s.loading = true;
    }
    request_world_tick();
}

/// The sheet closed. Starting fresh each time beats resuming a stale query —
/// the last thing a returning search almost never wants is last session's list.
pub fn search_close() {
    let s = search();
    s.query.clear();
    s.submitted = None;
    s.origin = None;
    s.near = None;
    s.loading = false;
    s.results.clear();
    fetch_sync(CATEGORY, &[]);
}

/// The search's turn: once typing has paused, a one-item wishlist for the query.
/// The next query's supersedes this one, and the scheduler's yield aborts it.
pub fn search_step() {
    let s = search();
    let q = s.query.trim().to_string();
    if q.is_empty() || s.submitted.as_deref() == Some(q.as_str()) || now() - s.typed_at < DEBOUNCE_MS {
        return;
    }
    s.near = use_map_store().map_center.map(|c| {
        let mut g = [0.0; 3];
        epsg4326::unproject(&mut g, c.i, c.f);
        (g[0], g[1])
    });
    fetch_sync(
        CATEGORY,
        &[Demand {
            key: format!("{CATEGORY}:{q}"),
            category: CATEGORY.to_string(),
            priority: PRIORITY,
        }],
    );
    s.submitted = Some(q);
}

/// The URL a started search demand is fetched from.
pub fn search_request_url(query: &str) -> String {
    search_url(query, search().near)
}

/// The shell's answer for `query`: the response body.
pub fn search_arrived(query: &str, body: Option<&str>) {
    let s = search();
    if s.submitted.as_deref() != Some(query) {
        return;
    }
    s.results = body.and_then(parse_search).unwrap_or_default();
    s.origin = s.near;
    s.loading = false;
}

/// The shell's report that the request for `query` failed or was aborted;
/// loading clears on a real failure, not when superseded.
pub fn search_failed(query: &str, aborted: bool) {
    let s = search();
    if !aborted && s.submitted.as_deref() == Some(query) {
        s.loading = false;
    }
}

/// Great-circle metres between two WGS84 points — not the world-unit distance
/// scaled by a metre per unit, which is a Mercator plane stretched by
/// 1/cos(latitude): 1.26x at Seoul's, not a rounding error on a label whose
/// whole job is to say how far away something is.
fn ground_distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lat2) = (a.1.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = (b.0 - a.0).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Whole metres under a kilometre, then one decimal, then whole kilometres —
/// the precision a reader can act on at each scale.
fn format_distance(m: f64) -> String {
    if m < 1000.0 {
        format!("{}m", m.round())
    } else if m < 10000.0 {
        format!("{:.1}km", m / 1000.0)
    } else {
        format!("{}km", group_thousands((m / 1000.0).round() as u64))
    }
}

/// One result as the sheet shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchRow {
    pub gers_id: String,
    pub kind: &'static str,
    pub name: String,
    /// Where the point is, resolved from its coordinates; empty with none.
    pub subtitle: String,
    pub distance: Option<String>,
    pub icon: String,
    pub color: String,
    pub lon: f64,
    pub lat: f64,
}

/// Whether a request is in flight, and the rows against `language`: divisions
/// first, as the backend orders them.
pub fn search_rows(language: &str) -> (bool, Vec<SearchRow>) {
    let s = search();
    let lang = language.split('-').next().unwrap_or(language).to_lowercase();
    let mut rows: Vec<SearchRow> = s
        .results
        .iter()
        .map(|r| {
            // A division's multilingual name against the UI language, falling
            // back to the given name; a POI has no `common`.
            let name = r.common.as_ref().and_then(|c| c.iter().find(|(k, _)| *k == lang)).map_or(r.name.clone(), |(_, v)| v.clone());
            let theme = resolve_poi_theme(&r.category);
            SearchRow {
                gers_id: r.gers_id.clone(),
                kind: match r.kind {
                    SearchKind::Division => "division",
                    SearchKind::Poi => "poi",
                },
                name,
                subtitle: if r.context.is_empty() { String::new() } else { assemble_label(&r.context, &lang) },
                distance: s.origin.map(|o| format_distance(ground_distance_m(o, (r.lon, r.lat)))),
                icon: theme.icon.to_string(),
                color: theme.color.to_string(),
                lon: r.lon,
                lat: r.lat,
            }
        })
        .collect();
    rows.sort_by_key(|r| r.kind != "division");
    (s.loading, rows)
}
