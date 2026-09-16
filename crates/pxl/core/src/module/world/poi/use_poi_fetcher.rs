// pxl_core/module/world/poi/use_poi_fetcher.rs

//! Demands POI for every page within the fetch range that isn't cached yet —
//! the same range terrain's fetcher loads at, derived from one render
//! distance, via the shared fetch scheduler so the two subsystems share one
//! concurrency quota. Evicts on terrain's own held set once pages fall out of
//! it: a single dense urban page can hold thousands of records uncapped, so
//! leaving the pages unbounded for a whole session's panning is a real memory
//! concern.
//!
//! Gated on the show_poi setting: hidden means an empty wishlist — no new
//! fetch, and any POI request still in flight is aborted by the scheduler's
//! own yield. Cached pages are left as-is so toggling back on feels instant for
//! whatever's still in range.

use pxl_shared::spec::space::pvf3::ivec::IVec;

use super::store::{clear_pois, get_pois, poi_pages};
use crate::app::setting::store::use_settings_store;
use crate::sys::global::Global;
use crate::module::world::frame::range::{page_priority, pages_to_hold, FETCH_HYSTERESIS};
use crate::module::world::frame::use_range_anchor::use_range_anchor;
use crate::module::world::sched::demand::Demand;
use crate::module::world::sched::demand::Category;
use crate::module::world::sched::fetch_scheduler::{fetch_running, fetch_sync, page_key};
use crate::module::world::terrain::config::{KEEP_DIST, RESIDENT_PAGES};

pub const CATEGORY: Category = Category::Poi;

// POI sits one behind terrain at equal page distance — the POI layer grounds
// each badge against that page's terrain, so fetching POI ahead of the ground
// it lands on just makes it wait.
const PRIORITY_BIAS: f64 = 1.0;

/// Evicts cached pages outside the held set, mirroring terrain's own cleanup.
fn cleanup_far_pois(anchor: IVec) {
    let held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES());
    // Collected first: clearing moves the table's last page into the hole.
    // What a run leaves uncollected, the next one takes.
    let mut evict: Vec<IVec> = Vec::new();
    evict.extend(poi_pages().keys().filter(|&p| !held.iter().any(|h| h.page_idx == p)));
    for page_idx in evict {
        clear_pois(page_idx);
    }
}

/// One Demand per uncached page within the fetch range — the scheduler dedups
/// against whatever it's already carrying, so this can be rebuilt in full on
/// every run.
fn poi_demands(anchor: IVec) -> Vec<Demand> {
    let mut demands = Vec::new();
    let mut add = |page_idx: IVec, dist2: f64| {
        if get_pois(page_idx).is_some() {
            return;
        }
        demands.push(Demand {
            key: page_key(CATEGORY, page_idx),
            priority: page_priority(dist2) + PRIORITY_BIAS,
        });
    };

    let held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES());
    for p in &held {
        add(p.page_idx, p.dist2);
    }
    // Same in-flight hysteresis as terrain — see FETCH_HYSTERESIS.
    for p in pages_to_hold(anchor, KEEP_DIST + FETCH_HYSTERESIS, RESIDENT_PAGES() + 16) {
        if !held.iter().any(|h| h.page_idx == p.page_idx) && fetch_running(page_key(CATEGORY, p.page_idx)) {
            add(p.page_idx, p.dist2);
        }
    }
    demands
}

/// What the last run saw: the effect's dependencies.
static LAST: Global<Option<(Option<IVec>, bool)>> = Global::new();

/// Runs when the range anchor or the setting changed — the effect's turn.
pub fn poi_fetcher_step() {
    let anchor = use_range_anchor();
    let show_poi = use_settings_store().show_poi;
    let last = LAST.get(|| None);
    if *last == Some((anchor, show_poi)) {
        return;
    }
    *last = Some((anchor, show_poi));

    let Some(anchor) = anchor.filter(|_| show_poi) else {
        fetch_sync(CATEGORY, &[]);
        return;
    };
    cleanup_far_pois(anchor);
    fetch_sync(CATEGORY, &poi_demands(anchor));
}
