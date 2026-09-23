// pxl-kernel/layer/poi/fetcher.rs

//! Demands POI for every leaf of terrain's held ring that isn't cached yet —
//! the leaves, since a place is grounded against a leaf's terrain — via the
//! shared fetch scheduler so the two subsystems share one concurrency quota. Evicts on terrain's own held set once pages fall out of
//! it: a single dense urban page can hold thousands of records uncapped, so
//! leaving the pages unbounded for a whole session's panning is a real memory
//! concern.
//!
//! Gated on the show_poi setting: hidden means an empty wishlist — no new
//! fetch, and any POI request still in flight is aborted by the scheduler's
//! own yield. Cached pages are left as-is so toggling back on feels instant for
//! whatever's still in range.

use pxl_spec::space::pvf3::ivec::IVec;

use crate::global::Global;
use crate::io::demand::{Category, Demand};
use crate::io::scheduler::{fetch_running, fetch_sync, page_key};
use crate::layer::poi::store::{clear_pois, get_pois, poi_pages};
use crate::layer::terrain::level::{ring, HELD_AXIS};
use crate::layer::terrain::tile_fetcher::leaf_priority;
use crate::shell::setting::store::settings_store;
use crate::world::frame::range_anchor::range_anchor;

pub const CATEGORY: Category = Category::Poi;

// POI sits one behind terrain at equal page distance — the POI layer grounds
// each badge against that page's terrain, so fetching POI ahead of the ground
// it lands on just makes it wait.
const PRIORITY_BIAS: f64 = 1.0;

/// The leaves terrain holds.
fn held_leaves(anchor: IVec, axis: u32) -> Vec<IVec> {
    ring(0, anchor, axis).into_iter().map(|p| p.page).collect()
}

/// Evicts cached pages outside the held set, mirroring terrain's own cleanup.
fn cleanup_far_pois(anchor: IVec) {
    let held = held_leaves(anchor, HELD_AXIS);
    // Collected first: clearing moves the table's last page into the hole.
    // What a run leaves uncollected, the next one takes.
    let mut evict: Vec<IVec> = Vec::new();
    evict.extend(poi_pages().keys().filter(|p| !held.contains(p)));
    for page_idx in evict {
        clear_pois(page_idx);
    }
}

/// One Demand per uncached page within the fetch range — the scheduler dedups
/// against whatever it's already carrying, so this can be rebuilt in full on
/// every run.
fn poi_demands(anchor: IVec) -> Vec<Demand> {
    let mut demands = Vec::new();
    let mut add = |page_idx: IVec| {
        if get_pois(page_idx).is_some() {
            return;
        }
        demands.push(Demand {
            key: page_key(CATEGORY, page_idx),
            priority: leaf_priority(anchor, page_idx) + PRIORITY_BIAS,
        });
    };

    let held = held_leaves(anchor, HELD_AXIS);
    for &p in &held {
        add(p);
    }
    // Same in-flight hysteresis as terrain's.
    for p in held_leaves(anchor, HELD_AXIS + 2) {
        if !held.contains(&p) && fetch_running(page_key(CATEGORY, p)) {
            add(p);
        }
    }
    demands
}

/// What the last run saw: the effect's dependencies.
static LAST: Global<Option<(Option<IVec>, bool)>> = Global::new();

/// Runs when the range anchor or the setting changed — the effect's turn.
pub fn poi_fetcher_step() {
    let anchor = range_anchor();
    let show_poi = settings_store().show_poi;
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
