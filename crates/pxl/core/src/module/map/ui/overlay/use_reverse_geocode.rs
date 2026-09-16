// pxl_core/module/map/ui/overlay/use_reverse_geocode.rs

//! Neighborhood label for the current map_center, baked per L3 page from
//! Overture's Divisions theme. Page-keyed like POI, so the "have we already
//! resolved this one" check is exact page equality.
//!
//! Stores the fetched result as its unjoined parts, not a display string —
//! assemble_label runs against the current UI language whenever the label is
//! read, so switching language re-orders an already-fetched label instantly
//! instead of needing a re-fetch.

use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::sys::global::{now, Global};
use crate::module::map::store::use_map_store;
use crate::module::world::geocode::label::{assemble_label, Label};
use crate::module::world::geocode::repository::{parse_geocode, PartTable, Rows};
use crate::module::world::sched::demand::Demand;
use crate::module::world::sched::demand::Category;
use crate::module::world::sched::fetch_scheduler::{fetch_sync, page_key};

// A short debounce still matters even though the real dedup key is an exact
// page: panning fires map_center updates every frame, and most of those land
// on the same page as the last one.
const DEBOUNCE_MS: f64 = 500.0;

pub const CATEGORY: Category = Category::Geocode;
// Ahead of every streaming category (terrain starts at 0, POI at 1) — this is a
// label the user is waiting to read, not background prefetch, and there is
// only ever one of it in flight.
const PRIORITY: f64 = -5.0;

/// What one answer can hold: a division chain is a handful of parts, each
/// with its translations.
const PARTS: usize = 32;
const NAMES: usize = 1024;

struct Geocode {
    /// The last answer, read in place.
    table: PartTable,
    parts: Option<Rows>,
    loading: bool,
    /// The page a request has been submitted for, not the page a label was
    /// received for. It exists to stop the same page being asked about twice,
    /// so it is released again the moment an attempt stops being one.
    submitted_page: Option<IVec>,
    /// The page the centre is on, and since the centre last moved.
    page: Option<(IVec, f64)>,
    /// The centre as last seen: what the debounce waits to stand still.
    last_center: Option<PVF3>,
}

static GEOCODE: Global<Geocode> = Global::new();

fn geocode() -> &'static mut Geocode {
    GEOCODE.get(|| Geocode {
        table: PartTable::new(PARTS, NAMES),
        parts: None,
        loading: false,
        submitted_page: None,
        page: None,
        last_center: None,
    })
}

/// The reverse geocode's turn: once the centre has stayed on a page for the
/// debounce, a one-item wishlist for it. Submitting a new page supersedes the
/// previous one, which the scheduler's own yield aborts — latest wins, sharing
/// one quota with terrain and POI.
pub fn reverse_geocode_step() {
    let g = geocode();
    let Some(center) = use_map_store().map_center else { return };
    let page_idx = L3_IDX(center.i);
    // The debounce waits on the centre standing still, not on the page: a pan
    // that crosses into a page and keeps going would otherwise ask about a page
    // it is already leaving, since the page under it does not change again
    // until it is gone.
    if g.last_center != Some(center) {
        g.last_center = Some(center);
        g.page = Some((page_idx, now()));
    }
    let Some((page, since)) = g.page else { return };
    if now() - since < DEBOUNCE_MS || g.submitted_page == Some(page) {
        return;
    }
    g.submitted_page = Some(page);
    g.loading = true;
    fetch_sync(
        CATEGORY,
        &[Demand {
            key: page_key(CATEGORY, page),
            priority: PRIORITY,
        }],
    );
}

/// The shell's answer for a page: its body, or None for a 404 (outside the
/// Divisions coverage).
pub fn reverse_geocode_arrived(page_idx: IVec, body: Option<&str>) {
    let g = geocode();
    g.parts = body.and_then(|body| parse_geocode(&mut g.table, body));
    let _ = page_idx;
    g.loading = false;
}

/// The shell's report that the page's request failed or was aborted. A failed
/// attempt is not an answer, so the page goes back to being unasked; an abort
/// of this very page is the same. Loading clears on a real failure, but not
/// when superseded — the demand that superseded this one already set it.
pub fn reverse_geocode_failed(page_idx: IVec, aborted: bool) {
    let g = geocode();
    if g.submitted_page == Some(page_idx) {
        g.submitted_page = None;
    }
    if !aborted {
        g.loading = false;
    }
}

#[derive(Clone, Debug)]
pub struct ReverseGeocodeResult<'a> {
    pub label: Option<Label<'a>>,
    pub loading: bool,
}

/// The label against `language`, and whether a request is in flight.
pub fn use_reverse_geocode(language: &str) -> ReverseGeocodeResult<'static> {
    let g: &'static _ = geocode();
    ReverseGeocodeResult {
        label: g.parts.map(|parts| assemble_label(&g.table, parts, language)),
        loading: g.loading,
    }
}
