// pxl_core/module/world/terrain/use_tile_fetcher.rs

//! Which terrain pages should be loaded right now: the demand and eviction
//! loop, run when the range anchor or the device's page moves, and map_status
//! following the terrain queue. The fetch itself is the shell's, for the
//! demand keys the scheduler starts.

use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::xy::TO_XY;

use super::config::{KEEP_DIST, RESIDENT_PAGES};
use super::resident::{release_terrain, QUARANTINE_MS};
use super::store::{tile_pages, use_tile_store};
use crate::global::Global;
use crate::module::map::navigation::use_map_navigation::use_map_navigation;
use crate::module::map::store::{use_map_store, MapStatus};
use crate::module::world::frame::range::{page_priority, pages_to_hold, FETCH_HYSTERESIS};
use crate::module::world::frame::use_range_anchor::use_range_anchor;
use crate::module::world::sched::demand::Demand;
use crate::module::world::sched::fetch_scheduler::{fetch_pending_count, fetch_revision, fetch_running, fetch_sync, page_key};

pub const CATEGORY: &str = "terrain";

// The device's own page, when it's outside the fetch range, still has to load
// so the user marker can ground itself — but it's genuinely far, so it queues
// behind everything near the camera rather than jumping the line.
const PINNED_PAGE_PRIORITY: f64 = 1_000.0;

/// Builds the terrain wishlist: one Demand per uncached page within the held
/// set, nearest first, plus the device page when it falls outside that set.
/// The scheduler dedups against what it already holds, so this is rebuilt in
/// full on every run.
fn terrain_demands(anchor: IVec, pinned_page_idx: Option<IVec>) -> Vec<Demand> {
    let store = use_tile_store();
    let mut demands = Vec::new();
    let mut add = |page_idx: IVec, priority: f64| {
        if store.get_tile(page_idx).is_some() {
            return;
        }
        demands.push(Demand {
            key: page_key(CATEGORY, page_idx),
            category: CATEGORY.to_string(),
            priority,
        });
    };

    let held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES());
    for p in &held {
        add(p.page_idx, page_priority(p.dist2));
    }
    // A request already running for a page that has only just left the held
    // set is kept in the wishlist rather than withdrawn, so a pan does not
    // cancel work it will ask for again — see FETCH_HYSTERESIS.
    for p in pages_to_hold(anchor, KEEP_DIST + FETCH_HYSTERESIS, RESIDENT_PAGES() + 16) {
        if !held.iter().any(|h| h.page_idx == p.page_idx) && fetch_running(&page_key(CATEGORY, p.page_idx)) {
            add(p.page_idx, page_priority(p.dist2));
        }
    }

    if let Some(pinned) = pinned_page_idx {
        add(pinned, PINNED_PAGE_PRIORITY);
    }
    demands
}

/// Evicts cached pages that fall outside the held set — exactly the set the
/// demands above ask for, so a page is never evicted while it is still being
/// fetched. The pinned (device-own) page is exempt: it is kept warm regardless
/// of camera distance so the user marker can always ground itself.
fn cleanup_far_tiles(anchor: IVec, pinned_page_idx: Option<IVec>) {
    let held: Vec<IVec> = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES()).into_iter().map(|p| p.page_idx).collect();
    // Collected first: clearing moves the table's last page into the hole.
    let evict: Vec<IVec> = tile_pages().keys().filter(|p| Some(*p) != pinned_page_idx && !held.contains(p)).collect();
    for page_idx in evict {
        release_terrain(page_idx, QUARANTINE_MS);
        use_tile_store().clear_tile(page_idx);
    }
}

struct Fetcher {
    last: Option<(IVec, Option<IVec>)>,
    fetch_revision: u32,
}

static FETCHER: Global<Fetcher> = Global::new();

/// The tile fetcher's turn.
pub fn tile_fetcher_step() {
    let f = FETCHER.get(|| Fetcher { last: None, fetch_revision: u32::MAX });

    // map_status follows the terrain queue: LOADING while anything terrain is
    // queued or in flight, SUCCESS once it drains. Derived from the scheduler
    // rather than tracked here, so a later pan that supersedes this batch can't
    // leave a stale SUCCESS behind.
    let revision = fetch_revision();
    if revision != f.fetch_revision {
        f.fetch_revision = revision;
        let map = use_map_store();
        if fetch_pending_count(CATEGORY) > 0 {
            map.set_map_status(MapStatus::Loading);
        } else if map.map_status == MapStatus::Loading {
            map.set_map_status(MapStatus::Success);
        }
    }

    // The device's own page has to stay loaded independently of the view
    // center: panning moves map_center away from the user's real GPS position,
    // and without this that page falls outside the held set and the user
    // marker silently vanishes until the camera pans back near it.
    let device_page_idx = use_map_navigation().device_coord.map(|c| TO_XY(L3_IDX(c.i)));
    let Some(anchor) = use_range_anchor() else { return };
    if f.last == Some((anchor, device_page_idx)) {
        return;
    }
    f.last = Some((anchor, device_page_idx));
    cleanup_far_tiles(anchor, device_page_idx);
    fetch_sync(CATEGORY, &terrain_demands(anchor, device_page_idx));
}
