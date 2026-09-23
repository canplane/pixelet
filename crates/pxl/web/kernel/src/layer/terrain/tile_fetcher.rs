// pxl-kernel/layer/terrain/tile_fetcher.rs

//! Which terrain pages should be loaded right now: the demand and eviction
//! loop, run when the range anchor, the device's page or the window moves,
//! and map_status following the terrain queue. The HTTP is the shell's, for
//! the demand keys the scheduler starts; what arrives is io/fetch.rs's.

use pxl_spec::bake::constant::PAGE_TOP_LEVEL;
use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::xy::TO_XY;

use crate::control::navigation::store::map_navigation;
use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Demand, DemandKey};
use crate::io::scheduler::{fetch_pending_count, fetch_running, fetch_sync};

use crate::layer::terrain::level::{centre_dist, held, neighbourhood, ring, ring_of, window, LevelPage, HELD_AXIS, RENDER_AXIS, WINDOW_LEVELS};
use crate::layer::terrain::resident::free_slot;
use crate::layer::terrain::store::{level_pages, tile_store};
use crate::layer::terrain::tile_repository::{adopt_folded, ocean_retain, ocean_wanted, OCEAN_CATEGORY};
use crate::shell::map_store::{map_store, MapStatus};
use crate::world::change::wake_at;
use crate::world::frame::range_anchor::range_anchor;

pub const CATEGORY: Category = Category::Terrain;

// The device's own page, when it's outside the fetch range, still has to load
// so the user marker can ground itself — but it's genuinely far, so it queues
// behind everything near the camera rather than jumping the line.
const PINNED_PAGE_PRIORITY: f64 = 1_000.0;

/// How long a failed ask for the shared ocean tile waits before the next, while
/// pages still wait on it.
const OCEAN_RETRY_MS: f64 = 2_000.0;

pub fn page_key(p: LevelPage) -> DemandKey {
    DemandKey::new(CATEGORY, p.subject())
}

/// Fetch order, coarse first: every page drawn from, coarsest level first,
/// then every rim page the same way, and within each the nearest first. A
/// coarse page is the fallback a fine one refines, so it is worth more early,
/// and it is small against the ground it covers; the rim is margin, wanted
/// only once the view moves. Doubled, as every page priority is, so POI can
/// interleave at the odd numbers, and the distance kept under one so it never
/// reaches them. A page coarser than the window's top — a request the window
/// has left running (terrain_demands) — ranks with the top.
fn priority(anchor: IVec, p: LevelPage, top: u32) -> f64 {
    let rim = (ring_of(anchor, p) > RENDER_AXIS / 2) as u32;
    let d = centre_dist(anchor, p);
    2.0 * (rim * WINDOW_LEVELS + top.saturating_sub(p.level)) as f64 + d / (d + 1.0)
}

/// Where a leaf stands in terrain's order, for what fetches beside it.
pub fn leaf_priority(anchor: IVec, page_idx: IVec) -> f64 {
    priority(anchor, LevelPage::new(0, page_idx), window().end - 1)
}

/// Makes every page of the window's rings the client can make from what it
/// holds, rather than asking for it: a page whose sixteen pages a level down
/// are all resident is folded here by the fold the bake runs (pxl-spec
/// bake/fold.rs), so it is the page the server would send, byte for byte. A
/// level at a time from the finest, so what one level folds feeds the next;
/// and the leaves a zoom out has just left the window with are still
/// resident, which is what the first level up is made of.
fn fold_held(anchor: IVec) {
    let store = tile_store();
    let w = window();
    for level in w.clone().filter(|&l| l > 0) {
        for p in held(level, anchor) {
            if store.get_page(p).is_some() {
                continue;
            }
            let around = neighbourhood(p);
            let Some(tiles) = around.iter().map(|&q| store.get_page(q)).collect::<Option<Vec<_>>>() else {
                continue;
            };
            let terrain: [&Terrain; 16] = std::array::from_fn(|k| &tiles[k].terrain);
            if adopt_folded(p, &fold_terrain(terrain, level)).is_err() {
                return;
            }
        }
    }
}

/// Builds the terrain wishlist: every page of every level's held set not yet
/// resident, plus the device page when it falls outside that set. The
/// scheduler dedups against what it already holds, so this is rebuilt in full
/// on every run.
fn terrain_demands(anchor: IVec, pinned_page_idx: Option<IVec>) -> Vec<Demand> {
    let store = tile_store();
    let w = window();
    let top = w.end - 1;
    let mut demands = Vec::new();
    let mut add = |p: LevelPage| {
        if store.get_page(p).is_none() {
            demands.push(Demand {
                key: page_key(p),
                priority: priority(anchor, p, top),
            });
        }
    };
    for level in w.clone().rev() {
        let wanted = held(level, anchor);
        for &p in &wanted {
            add(p);
        }
        // A request already running for a page that has only just left the
        // held set is kept in the wishlist rather than withdrawn, so a pan
        // does not cancel work it will ask for again. One page further out: a
        // pan cannot carry a page that far before its request settles, while
        // a jump, which moves by many pages, still yields as designed.
        for p in ring(level, anchor, HELD_AXIS + 2) {
            if !wanted.contains(&p) && fetch_running(page_key(p)) {
                add(p);
            }
        }
    }
    // The same for the levels a zoom in has just taken the window below: a
    // coarse page is what a zoom back out draws first, and it is small. Only
    // as far as the record groups the window leaves free, since a page lands
    // in its level's group (fixed.rs) and one sharing a held level's would
    // take a held page's record (tile_repository.rs refuses it).
    for level in w.end..(w.start + WINDOW_LEVELS).min(PAGE_TOP_LEVEL + 1) {
        for p in ring(level, anchor, HELD_AXIS + 2) {
            if fetch_running(page_key(p)) {
                add(p);
            }
        }
    }
    if let Some(pinned) = pinned_page_idx {
        let p = LevelPage::new(0, pinned);
        if store.get_page(p).is_none() && !demands.iter().any(|d| d.key == page_key(p)) {
            demands.push(Demand {
                key: page_key(p),
                priority: PINNED_PAGE_PRIORITY,
            });
        }
    }
    demands
}

/// Whether letting `p` go leaves its ground drawn: the page it folds into is
/// resident, or the four it is folded from are.
fn covered(p: LevelPage) -> bool {
    let store = tile_store();
    store.get_page(p.parent()).is_some() || (p.level > 0 && p.children().iter().all(|&c| store.get_page(c).is_some()))
}

/// Evicts the pages of the window's levels that fall outside their level's
/// held set — exactly the set the demands above ask for, so a page is never
/// evicted while it is still being fetched — once something else draws their
/// ground. One nothing else covers yet stays: it is what select draws from
/// until the page over it arrives, and a page whose record another takes
/// meanwhile is let go then (tile_repository.rs). A level the window has
/// left is not evicted: its record group is the one the level it took
/// shares, so its pages go as that level's arrive, and until then they are
/// what is drawn. The pinned (device-own) page is exempt: it is kept warm
/// regardless of camera distance so the user marker can always ground itself.
fn cleanup_far_tiles(anchor: IVec, pinned_page_idx: Option<IVec>) {
    let pinned = pinned_page_idx.map(|p| LevelPage::new(0, p));
    let w = window();
    let wanted: Vec<LevelPage> = w.clone().flat_map(|level| held(level, anchor)).collect();
    ocean_retain(|p| Some(p) == pinned || wanted.contains(&p));
    for level in w {
        // Collected first: clearing moves the table's last page into the hole.
        let evict: Vec<LevelPage> = level_pages(level)
            .keys()
            .map(|page| LevelPage::new(level, page))
            .filter(|p| Some(*p) != pinned && !wanted.contains(p) && covered(*p))
            .collect();
        for p in evict {
            free_slot(p);
            tile_store().clear_page(p);
        }
    }
}

struct Fetcher {
    last: Option<(IVec, Option<IVec>, (u32, u32))>,
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
    if wanted && !pending && now() < f.ocean_ask_at {
        wake_at(f.ocean_ask_at);
    } else if wanted && !pending {
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
    let w = window();
    let levels = (w.start, w.end);
    match f.retry_at {
        Some(at) if now() >= at => {
            f.retry_at = None;
            f.last = None;
        }
        Some(at) => wake_at(at),
        None => {}
    }
    if f.last == Some((anchor, device_page_idx, levels)) {
        return;
    }
    f.last = Some((anchor, device_page_idx, levels));
    cleanup_far_tiles(anchor, device_page_idx);
    fold_held(anchor);
    // A demand the queue had no row for is asked for on the next turn: the
    // wishlist is otherwise given only when the view moves, and a view at
    // rest would keep the hole.
    if fetch_sync(CATEGORY, &terrain_demands(anchor, device_page_idx)) > 0 {
        f.last = None;
    }
}
