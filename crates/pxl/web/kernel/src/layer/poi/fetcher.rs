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
//!
//! Not wired: nothing calls `poi_fetcher_step`, so no POI page is requested
//! and the fetch queue's rows are terrain's (io/scheduler.rs sizes them so).
//! Kept whole for when POI comes back; wiring it back means the queue holds
//! its leaves' ring too.

use pxl_spec::space::pvf3::ivec::IVec;

use crate::io::demand::Category;
use crate::io::policy;
use crate::io::sched::declare;
use crate::layer::poi::store::{clear_pois, get_pois, poi_pages};
use crate::layer::terrain::level::{centre_dist, ring, LevelPage, HELD_AXIS};
use crate::shell::setting::store::settings_store;
use crate::world::frame::range_anchor::range_anchor;

pub const CATEGORY: Category = Category::Poi;

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

/// Every page in the fetch range with no places yet, and how far each is.
/// Said in full every run: a set that is the same set changes nothing.
fn poi_wants(anchor: IVec) -> Vec<(u64, crate::io::sched::Rank)> {
    held_leaves(anchor, HELD_AXIS)
        .into_iter()
        .filter(|&p| get_pois(p).is_none())
        .map(|p| {
            let at = LevelPage::new(0, p);
            (p, policy::poi(centre_dist(anchor, at) * at.span()))
        })
        .collect()
}

/// Runs every turn: what the badges need and do not have.
pub fn poi_fetcher_step() {
    let Some(anchor) = range_anchor().filter(|_| settings_store().show_poi) else {
        declare(CATEGORY, &[]);
        return;
    };
    cleanup_far_pois(anchor);
    declare(CATEGORY, &poi_wants(anchor));
}
