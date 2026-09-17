// pxl_core/module/map/ui/overlay/search_modal.rs

//! Forward search, everything but the sheet it is shown in: the typed query
//! debounced, one request in flight, the results read, and each row's name,
//! where it is and how far away, as the UI renders them. Always the live API
//! (search/repository.rs); the request is the shell's.

use core::fmt::{self, Write};

use pxl_shared::spec::space::adapter::epsg4326;
use xpute_core::kit::hash::fnv1a64_str;

use crate::module::map::store::use_map_store;
use crate::module::world::change::request_world_tick;
use crate::module::world::geocode::label::{assemble_label, primary_lang, Label};
use crate::module::world::poi::poi_theme::resolve_poi_theme;
use crate::module::world::sched::demand::{Category, Demand, DemandKey};
use crate::module::world::sched::fetch_scheduler::fetch_sync;
use crate::module::world::search::repository::{parse_search, search_url, SearchKind, SearchTable, SearchUrl};
use crate::sys::global::{now, Global};

// Debounced like use_reverse_geocode.rs's own DEBOUNCE_MS, but shorter: that
// one waits for panning to settle on a new page; this waits for typing to
// pause, a much shorter rhythm for a search box.
const DEBOUNCE_MS: f64 = 300.0;

pub const CATEGORY: Category = Category::Search;
// The most latency-sensitive fetch in the app — a typed query with the user
// staring at an empty result list — so it outranks even geocode (-5) and
// every streaming category. Single-slot, so it never occupies more than one of
// the fetch scheduler's quota slots.
const PRIORITY: f64 = -10.0;

const EARTH_RADIUS_M: f64 = 6_378_137.0;

type Query = String;

struct Search {
    query: Query,
    typed_at: f64,
    /// The query a request was submitted for.
    submitted: Option<Query>,
    /// Where the request ranks from, frozen when it was submitted: the same
    /// point the API ranked against, so the distances shown always agree with
    /// the order they are shown in, and a camera still drifting behind the open
    /// sheet cannot renumber a list being read.
    near: Option<(f64, f64)>,
    origin: Option<(f64, f64)>,
    loading: bool,
    results: SearchTable,
}

static SEARCH: Global<Search> = Global::new();

fn search() -> &'static mut Search {
    SEARCH.get(|| Search {
        query: Query::new(),
        typed_at: 0.0,
        submitted: None,
        near: None,
        origin: None,
        loading: false,
        results: SearchTable::new(),
    })
}

/// The query as typed. An empty one drops whatever the last keystroke left in
/// flight.
pub fn search_query(query: &str) {
    let s = search();
    if s.query != query {
        // A keystroke drops whatever is in flight, as the sheet's own cleanup
        // did: an answer to what was typed before is not an answer to what is
        // typed now, and it would land on the rows under the new query.
        s.submitted = None;
        fetch_sync(CATEGORY, &[]);
    }
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
    let q: Query = s.query.trim().to_string();
    if q.is_empty() || s.submitted.as_ref() == Some(&q) || now() - s.typed_at < DEBOUNCE_MS {
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
            key: query_key(&q),
            priority: PRIORITY,
        }],
    );
    s.submitted = Some(q);
}

/// A query's demand key. The query stands in it as its hash, so a query of
/// any length is one key; its text is the modal's (`submitted`).
fn query_key(query: &str) -> DemandKey {
    DemandKey::new(CATEGORY, fnv1a64_str(query))
}

/// Whether a demand's subject is the query submitted last.
fn is_submitted(s: &Search, subject: u64) -> bool {
    s.submitted.as_deref().is_some_and(|q| fnv1a64_str(q) == subject)
}

/// The URL the submitted search is fetched from.
pub fn search_request_url() -> SearchUrl<'static> {
    let s: &'static Search = search();
    search_url(s.submitted.as_deref().unwrap_or(""), s.near)
}

/// The shell's answer for the demand whose subject is `subject`: the
/// response body.
pub fn search_arrived(subject: u64, body: Option<&str>) {
    let s = search();
    if !is_submitted(s, subject) {
        return;
    }
    // No body at all is an empty answer; a body that does not parse leaves the
    // rows alone (parse_search reads before it clears).
    match body {
        Some(body) => {
            parse_search(&mut s.results, body);
        }
        None => s.results.clear(),
    }
    s.origin = s.near;
    s.loading = false;
}

/// The shell's report that the request for `query` failed or was aborted;
/// loading clears on a real failure, not when superseded.
pub fn search_failed(subject: u64, aborted: bool) {
    let s = search();
    if !aborted && is_submitted(s, subject) {
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

/// A distance as the sheet says it: whole metres under a kilometre, then one
/// decimal, then whole kilometres grouped by thousands — the precision a
/// reader can act on at each scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distance(pub f64);

impl fmt::Display for Distance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.0;
        if m < 1000.0 {
            return write!(f, "{}m", m.round());
        }
        if m < 10000.0 {
            // Rounded half away from zero, which is what the sheet has always
            // shown: Rust's own `{:.1}` takes a tie to even, and 1250 m would
            // read as 1.2km where every earlier build said 1.3km.
            return write!(f, "{:.1}km", (m / 100.0).round() / 10.0);
        }
        let km = (m / 1000.0).round() as u64;
        let (mut digits, mut len, mut v) = ([0u8; 20], 0, km);
        loop {
            digits[len] = b'0' + (v % 10) as u8;
            len += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        for k in 0..len {
            if k > 0 && (len - k).is_multiple_of(3) {
                f.write_str(",")?;
            }
            f.write_char(digits[len - 1 - k] as char)?;
        }
        f.write_str("km")
    }
}

/// One result as the sheet shows it, borrowed from the results.
#[derive(Clone, Debug)]
pub struct SearchRow<'a> {
    pub gers_id: &'a str,
    pub kind: &'static str,
    pub name: &'a str,
    /// Where the point is, resolved from its coordinates; empty with none.
    pub subtitle: Label<'a>,
    pub distance: Option<Distance>,
    pub icon: &'static str,
    pub color: &'static str,
    pub lon: f64,
    pub lat: f64,
}

/// Each row against `language` handed to `row`, divisions first as the
/// backend orders them; whether a request is in flight.
pub fn search_rows(language: &str, mut row: impl FnMut(SearchRow<'_>)) -> bool {
    let s = search();
    let lang = primary_lang(language);
    let t = &s.results.parts;
    for divisions in [true, false] {
        for r in s.results.results().iter().filter(|r| matches!(r.kind, SearchKind::Division) == divisions) {
            let theme = resolve_poi_theme(&r.category);
            row(SearchRow {
                gers_id: &r.gers_id,
                kind: if divisions { "division" } else { "poi" },
                // A division's multilingual name against the UI language,
                // falling back to the given name; a POI has no `common`.
                name: t.name_in(r.common, &r.name, &lang),
                subtitle: assemble_label(t, r.context, &lang),
                distance: s.origin.map(|o| Distance(ground_distance_m(o, (r.lon, r.lat)))),
                icon: theme.icon,
                color: theme.color,
                lon: r.lon,
                lat: r.lat,
            });
        }
    }
    s.loading
}
