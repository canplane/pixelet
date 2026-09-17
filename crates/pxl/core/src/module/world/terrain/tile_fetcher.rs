// pxl_core/module/world/terrain/tile_fetcher.rs

//! Which terrain pages should be loaded right now: the demand and eviction
//! loop, run when the range anchor or the device's page moves, and map_status
//! following the terrain queue. The HTTP is the shell's, for the demand keys
//! the scheduler starts; what arrives is module/world/fetch.rs's.

use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::xy::TO_XY;

use super::config::{KEEP_DIST, RESIDENT_PAGES};
use super::resident::free_slot;
use super::store::{tile_pages, tile_store};
use super::tile_repository::{ocean_retain, ocean_wanted, OCEAN_CATEGORY};
use crate::module::map::navigation::map_navigation::map_navigation;
use crate::module::map::store::{map_store, MapStatus};
use crate::module::world::frame::range_anchor::range_anchor;
use crate::module::world::sched::demand::{Category, Demand, DemandKey};
use crate::module::world::sched::fetch_scheduler::{fetch_pending_count, fetch_running, fetch_sync, page_key};
use crate::render::chunk_range::{page_priority, pages_to_hold, FETCH_HYSTERESIS};
use crate::sys::global::{now, Global};

pub const CATEGORY: Category = Category::Terrain;

// The device's own page, when it's outside the fetch range, still has to load
// so the user marker can ground itself — but it's genuinely far, so it queues
// behind everything near the camera rather than jumping the line.
const PINNED_PAGE_PRIORITY: f64 = 1_000.0;

/// How long a failed ask for the shared ocean tile waits before the next, while
/// pages still wait on it.
const OCEAN_RETRY_MS: f64 = 2_000.0;

/// Builds the terrain wishlist: one Demand per uncached page within the held
/// set, nearest first, plus the device page when it falls outside that set.
/// The scheduler dedups against what it already holds, so this is rebuilt in
/// full on every run.
fn terrain_demands(anchor: IVec, pinned_page_idx: Option<IVec>) -> Vec<Demand> {
    let store = tile_store();
    let mut demands = Vec::new();
    let mut add = |page_idx: IVec, priority: f64| {
        if store.get_tile(page_idx).is_some() {
            return;
        }
        demands.push(Demand {
            key: page_key(CATEGORY, page_idx),
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
        if !held.iter().any(|h| h.page_idx == p.page_idx) && fetch_running(page_key(CATEGORY, p.page_idx)) {
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
    let held = pages_to_hold(anchor, KEEP_DIST, RESIDENT_PAGES());
    let holds = |p: IVec| held.iter().any(|h| h.page_idx == p);
    ocean_retain(|p| Some(p) == pinned_page_idx || holds(p));
    // Collected first: clearing moves the table's last page into the hole.
    // What a run leaves uncollected, the next one takes.
    let mut evict: Vec<IVec> = Vec::new();
    evict.extend(tile_pages().keys().filter(|&p| Some(p) != pinned_page_idx && !holds(p)));
    for page_idx in evict {
        free_slot(page_idx);
        tile_store().clear_tile(page_idx);
    }
}

struct Fetcher {
    last: Option<(IVec, Option<IVec>)>,
    ocean_ask_at: f64,
    /// When a page that failed is asked for again. The wishlist is rebuilt
    /// only when the anchor moves, and the scheduler drops a failed demand, so
    /// a page whose fetch failed while the view stood still would stay a hole
    /// until the next pan.
    retry_at: Option<f64>,
}

static FETCHER: Global<Fetcher> = Global::new();

fn fetcher() -> &'static mut Fetcher {
    FETCHER.get(|| Fetcher {
        last: None,
        ocean_ask_at: 0.0,
        retry_at: None,
    })
}

/// A terrain page's fetch failed: the wishlist is rebuilt after the same wait
/// the shared tile takes, not at once, which would ask again into whatever
/// turned it away.
pub fn terrain_failed() {
    let f = fetcher();
    f.retry_at.get_or_insert(now() + OCEAN_RETRY_MS);
}

/// Asks for the shared ocean tile while pages wait on it and nothing is asking
/// already, and withdraws the ask once none do.
fn ocean_step(f: &mut Fetcher) {
    let wanted = ocean_wanted();
    let pending = fetch_pending_count(OCEAN_CATEGORY) > 0;
    if wanted && !pending && now() >= f.ocean_ask_at {
        f.ocean_ask_at = now() + OCEAN_RETRY_MS;
        // The stream has one member, so its subject names nothing.
        let demand = Demand {
            key: DemandKey::new(OCEAN_CATEGORY, 0),
            priority: 0.0,
        };
        fetch_sync(OCEAN_CATEGORY, std::slice::from_ref(&demand));
    } else if !wanted && pending {
        fetch_sync(OCEAN_CATEGORY, &[]);
    }
}

/// The tile fetcher's turn.
pub fn tile_fetcher_step() {
    let f = fetcher();
    ocean_step(f);

    // map_status follows the terrain queue: LOADING while anything terrain is
    // queued or in flight, SUCCESS once it drains. Derived from the scheduler
    // rather than tracked here, so a later pan that supersedes this batch can't
    // leave a stale SUCCESS behind.
    let map = map_store();
    if fetch_pending_count(CATEGORY) + fetch_pending_count(OCEAN_CATEGORY) > 0 {
        map.map_status = MapStatus::Loading;
    } else if map.map_status == MapStatus::Loading {
        map.map_status = MapStatus::Success;
    }

    // The device's own page has to stay loaded independently of the view
    // center: panning moves map_center away from the user's real GPS position,
    // and without this that page falls outside the held set and the user
    // marker silently vanishes until the camera pans back near it.
    let device_page_idx = map_navigation().device_coord.map(|c| TO_XY(L3_IDX(c.i)));
    let Some(anchor) = range_anchor() else { return };
    if f.retry_at.is_some_and(|at| now() >= at) {
        f.retry_at = None;
        f.last = None;
    }
    if f.last == Some((anchor, device_page_idx)) {
        return;
    }
    f.last = Some((anchor, device_page_idx));
    cleanup_far_tiles(anchor, device_page_idx);
    fetch_sync(CATEGORY, &terrain_demands(anchor, device_page_idx));
}
