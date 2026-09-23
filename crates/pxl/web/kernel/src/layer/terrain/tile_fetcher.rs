// pxl-kernel/layer/terrain/tile_fetcher.rs

//! Which terrain pages should be loaded right now: the demand and eviction
//! loop, run when the range anchor, the device's page or the window moves,
//! and map_status following the terrain queue. The HTTP is the shell's, for
//! the demand keys the scheduler starts; what arrives is io/fetch.rs's.

use pxl_spec::bake::constant::PAGE_TOP_LEVEL;
use pxl_spec::bake::fold::fold_terrain_held;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::xy::TO_XY;

use crate::control::navigation::store::map_navigation;
use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Demand, DemandKey};
use crate::io::scheduler::{fetch_pending_count, fetch_sync};
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::layer::terrain::level::{centre_dist, centre_page, held, neighbourhood, ring_of, window, LevelPage, RENDER_AXIS};
use crate::layer::terrain::resident::holds_record;
use crate::layer::terrain::resolve::{resolve, Resolved};
use crate::layer::terrain::store::{level_pages, tile_store};
use crate::layer::terrain::tile_repository::{adopt_folded, ocean_retain, ocean_wanted, OCEAN_CATEGORY};
use crate::shell::map_store::{map_store, MapStatus};
use crate::world::change::{wake_at, world_edge};
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

/// What a page's fetch is for. The classes do not trade against each other:
/// every fetch that puts ground on the screen runs before any that improves
/// ground already there, and both before anything bought against a move the
/// view has not made. It is the whole of the scheduling relation — a page
/// nobody is waiting on has no demand at all, since the resolver answered it
/// (resolve.rs).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    /// Nothing draws this ground.
    Coverage,
    /// Something coarser draws it; the fetch makes it sharp.
    Refinement,
    /// The rim, which is margin against the view moving.
    Prefetch,
}

/// Fetch order: the class, then the nearest the ground the view is looking
/// at. Why those and no others is the one order every scheduler in the tick
/// keeps (sched/priority.rs).
///
/// The distance is in world units, not in the page's own spans. In its own
/// spans it says how large the page is on the screen, which the ladder makes
/// about the same for every level — so a page at the horizon ranked with the
/// one under the camera, and the ring that quantized it carried the same
/// fault a ring at a time. Asking from the middle out is what puts the
/// ground anyone is looking at on the screen first, and the request order is
/// upstream of every other: a chunk cannot be mounted over a page that has
/// not come, and cannot be built before it is mounted.
///
/// Nothing under the distance, so nothing has to be discrete. The level term
/// is gone with the ring: it said a coarse page is the fallback a fine one
/// refines, which is true and is what the *class* says — a page with nothing
/// over it is Coverage and goes first whatever its level.
///
/// Doubled, so POI can interleave at the odd numbers, and the distance
/// squashed under one so it never reaches them. `d/(d + 1)` is monotone in
/// `d` and a double has the digits to keep pages apart at the far end, where
/// the ring's outermost is some tens of thousands of units out.
fn priority(anchor: IVec, p: LevelPage, want: Want) -> f64 {
    let d = centre_dist(anchor, p) * p.span();
    2.0 * want as u32 as f64 + d / (d + 1.0)
}

/// Past every priority `priority` produces: its top term is the class
/// doubled and the distance under it never reaches one, so a step past the
/// last class is past all of them. What fetches beside the ground but must
/// never take a credit from it.
pub const BEHIND_TERRAIN: f64 = 2.0 * (Want::Prefetch as u32 + 1) as f64;

/// Where a leaf stands in terrain's order, for what fetches beside it.
pub fn leaf_priority(anchor: IVec, page_idx: IVec) -> f64 {
    priority(anchor, LevelPage::new(0, page_idx), Want::Coverage)
}

/// Stands one page up from the four under it, if any of the window's wants
/// can be (pxl-spec bake/fold.rs). Whether it found one.
///
/// A level at a time from the finest, so what one fold makes feeds the next;
/// and the pages a zoom out has just left the window with are still resident,
/// which is what the first level up is made of.
///
/// The ring around those four is read where it is held and stood in for where
/// it is not, which is the whole of what separates this from the bake's own
/// fold: nothing inside the page moves, and its outermost padding sample —
/// the fourth tap of a bicubic read at the page's rim — is the block's edge
/// extended instead of the neighbour's. Requiring the ring instead would put
/// this at one page a level: a level-L page needs level L−1 across
/// `[2X−1, 2X+2]`, and a ring HELD_AXIS wide has that for its centre page
/// alone, so a zoom out fetched twenty-four pages in twenty-five it was
/// already holding the ground for.
fn fold_one(anchor: IVec) -> bool {
    let store = tile_store();
    let w = window();
    for level in w.clone().filter(|&l| l > 0) {
        for p in held(level, anchor) {
            if store.get_page(p).is_some() {
                continue;
            }
            let around = neighbourhood(p).map(|q| store.get_page(q).map(|t| &t.terrain));
            let Some(folded) = fold_terrain_held(around, level) else { continue };
            return adopt_folded(p, &folded).is_ok();
        }
    }
    false
}

static FOLDS: Global<Notify> = Global::new();

fn folds() -> &'static mut Notify {
    FOLDS.get(Notify::new)
}

/// The folds' task: one page a step, on what the turn's quota leaves after
/// the tick, for as long as there is one to make.
///
/// A fold is work — a page of ground voted cell by cell — and work goes on
/// the budgeted path. It used to run in the fetcher's turn, which is where
/// the wishlist is decided and nothing is budgeted, and a zoom out that made
/// a dozen pages foldable at once spent seventy milliseconds there in one
/// frame with the tick at one and a half (measured 2026-09-23, by removal).
/// The fetcher says what is wanted; what is done about it is paced like
/// everything else the kernel does.
///
/// Nothing waits on it. `resolve` calls a page whose four children are held
/// `Foldable` and the wishlist skips it either way, so a fold landing later
/// costs a page nothing but the moment it is drawn coarse.
pub async fn fold_task() {
    loop {
        notified(folds).await;
        loop {
            yield_if_spent(world_edge).await;
            let Some(anchor) = range_anchor() else { break };
            if !fold_one(anchor) {
                break;
            }
        }
    }
}

/// Builds the terrain wishlist: what the resident pages could not answer.
///
/// The desired set is the window's rings, and it is this whole function's
/// input — recomputed from the camera, never carried over. What a page needs
/// from the network is whatever the resolver could not do for it: a page that
/// is resident, or that the pages under it can be folded into, is not asked
/// for at all, and the rest are asked for in the class of what their absence
/// costs. That ordering is the design: zoom out spends no request on ground
/// the client is already holding at a finer level, and where a request is
/// needed the screen is already covered by a coarser page, so its latency is
/// ground sharpening rather than ground arriving.
///
/// While the view is being moved, only what nothing draws is asked for. The
/// desired set still changes every turn — the refinement is wanted, it is
/// simply not admitted — so a wheel that walks four levels in half a second
/// spends its credits on the ground under the hand rather than on sharpening
/// levels it is passing through.
fn terrain_demands(anchor: IVec, pinned_page_idx: Option<IVec>, moving: bool) -> Vec<Demand> {
    let w = window();
    let mut demands = Vec::new();
    let mut add = |p: LevelPage| {
        let want = match resolve(p, &w) {
            Resolved::Exact | Resolved::Foldable => return,
            _ if ring_of(anchor, p) > RENDER_AXIS / 2 => Want::Prefetch,
            Resolved::Missing => Want::Coverage,
            Resolved::Covered => Want::Refinement,
        };
        if moving && want != Want::Coverage {
            return;
        }
        demands.push(Demand {
            key: page_key(p),
            priority: priority(anchor, p, want),
        });
    };
    for level in w.clone().rev() {
        for p in held(level, anchor) {
            add(p);
        }
    }
    if let Some(pinned) = pinned_page_idx {
        let p = LevelPage::new(0, pinned);
        if tile_store().get_page(p).is_none() && !demands.iter().any(|d| d.key == page_key(p)) {
            demands.push(Demand {
                key: page_key(p),
                priority: PINNED_PAGE_PRIORITY,
            });
        }
    }
    demands
}

/// Lets go of the pages the camera has left that carry no bytes of their own.
///
/// A page with a record is not touched here. The range is what bounds it, and
/// which pages the range keeps is decided where a page arrives and needs room
/// (tile_repository.rs's `worth`) — the camera leaving a page is not a reason
/// to throw it away, it is a reason to let the next page that needs a record
/// have that one. Pan back, zoom back and the fold are all made of pages the
/// camera has left.
///
/// What is left here is the open-ocean pages: a page the bucket has no object
/// for shares one record with every other, so nothing will ever want its room
/// and it would sit in the table for the session. Its ground is not lost with
/// it — the level over it draws that — and asking again costs a 404. The
/// pinned (device-own) page is exempt: it is kept warm regardless of camera
/// distance so the user marker can always ground itself.
fn drop_aliases(anchor: IVec, pinned_page_idx: Option<IVec>) {
    let pinned = pinned_page_idx.map(|p| LevelPage::new(0, p));
    let w = window();
    let wanted: Vec<LevelPage> = w.clone().flat_map(|level| held(level, anchor)).collect();
    let keep = |p: LevelPage| Some(p) == pinned || wanted.contains(&p);
    ocean_retain(keep);
    for level in 0..=PAGE_TOP_LEVEL {
        // Collected first: clearing moves the table's last page into the hole.
        let drop: Vec<LevelPage> = level_pages(level).keys().map(|page| LevelPage::new(level, page)).filter(|p| !keep(*p) && !holds_record(*p)).collect();
        for p in drop {
            tile_store().clear_page(p);
        }
    }
}

/// What the wishlist is a function of. Two turns agreeing on all of it ask
/// for the same pages in the same order, so the second is skipped; this is an
/// equality on the inputs and never a wait for anything.
type Wish = (LevelPage, Option<IVec>, (u32, u32), bool);

struct Fetcher {
    last: Option<Wish>,
    ocean_ask_at: f64,
    /// When a page that failed is asked for again. The wishlist is rebuilt
    /// only when what it is a function of changes, and the scheduler drops a
    /// failed demand, so a page whose fetch failed while the view stood still
    /// would stay a hole until the next pan.
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
    // Keyed on the page the anchor stands in at the window's base, which
    // every ring of the window is centred on (level.rs `ring`): the pages of
    // every level of the window are that page shifted right, so a base page
    // that has not changed is a desired set that has not changed, and its
    // order is a nearest-first that a page's travel within one does not move
    // enough to ask again for.
    let base = centre_page(w.start, anchor);
    let moving = crate::sched::turn::interactive();

    // The wishlist every turn, and not only when the camera has moved.
    //
    // What a page needs from the network is what the resident pages could
    // not answer (resolve.rs), so a page arriving changes the answer for its
    // parent and its four children — one stops being wanted, the others stop
    // being what nothing draws and become what is merely coarse. Given only
    // when the camera moved, those kept the class they were asked under
    // until it did: work that had lost its reason to be first stayed first.
    // It is a hundred and twenty-five resolves, and the queue takes the list
    // as the whole truth either way.
    fetch_sync(CATEGORY, &terrain_demands(anchor, device_page_idx, moving));

    // The rest is a function of the camera alone, and costs a walk of the
    // page tables and a fold apiece.
    let wish = (base, device_page_idx, levels, moving);
    if f.last == Some(wish) {
        return;
    }
    f.last = Some(wish);
    drop_aliases(anchor, device_page_idx);
    folds().notify();
}

#[cfg(test)]
#[path = "tile_fetcher.test.rs"]
mod test;
