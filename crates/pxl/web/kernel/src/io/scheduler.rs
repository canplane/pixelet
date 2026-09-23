// pxl-kernel/io/scheduler.rs

//! The world's fetches, on the kernel's I/O queue (xpute_runtime::io::queue):
//! what a demand is called — `<category>:<page>`, or what stands for a search
//! query — and the ledger's record of each, over a queue that knows keys as
//! numbers.
//!
//! The quota is the central scheduler's: it grants the credits each turn
//! (`set_fetch_credits`), and the pump at the turn's end starts no more
//! than that many at once, and takes a credit back from a request nobody
//! wants where a demand has none. What the pump started and what it told to
//! stop are the shell's to take (`fetch_take_started`, `fetch_take_aborts`);
//! the shell runs each and settles it (`fetch_settle`). The scheduler never
//! touches a store.

use pxl_spec::space::pvf3::ivec::IVec;
pub use xpute_runtime::io::queue::IoCounts as FetchCounts;
use xpute_runtime::io::queue::{Demand as Queued, IoQueue};

use crate::global::Global;
use crate::io::demand::{Category, Demand, DemandKey, Phases};
use crate::layer::terrain::level::{HELD_AXIS, WINDOW_LEVELS};
use crate::sched::ledger::record::{ledger_record, Failure, Outcome, TaskRecord};

/// Fetches in flight at most, whatever the host grants.
const MAX_CREDITS: u32 = 16;

/// Terrain's wishlist at its widest: every page of every level's held ring,
/// and the device's own page when it falls outside them.
const TERRAIN_WANTED: usize = (WINDOW_LEVELS * HELD_AXIS * HELD_AXIS) as usize + 1;

/// The shared open-ocean tile, which is a stream of one.
const OCEAN_WANTED: usize = 1;

/// Demands wanted at once, which is the *sum* of the wishlists and not the
/// largest of them: each is replaced whole and they are replaced one at a
/// time, so a category filling the queue would have the next one's demands
/// refused — a current demand that cannot be represented, which is the one
/// thing this number exists to make impossible. Wiring POI back adds its
/// leaves' ring here (layer/poi/fetcher.rs).
///
/// Nothing is added for what is still in flight after its page left a
/// wishlist. That is not wanted, so it is not here: what is on the wire is a
/// set of its own and takes none of this room (xpute_runtime::io::queue).
const HELD: usize = TERRAIN_WANTED + OCEAN_WANTED;

/// Keys named at once: one per demand wanted, one per demand on the wire, and
/// one per abort not yet handed out.
const NAMES: usize = HELD * 2;

/// Categories: a handful.
const CATEGORIES: usize = 16;

struct Scheduler {
    queue: IoQueue<HELD>,
    credits: u32,
    /// The queue holds a category as its index here.
    categories: [Option<Category>; CATEGORIES],
    category_count: usize,
    /// Every key the queue holds or has aborted, by its 64-bit hash — the one
    /// word the queue orders by. A second key under a hash already taken is
    /// refused rather than confused with the first, which is why the pair is
    /// kept beside the hash and compared; not expected among tens of keys.
    names: [(u64, DemandKey); NAMES],
    name_count: usize,
}

/// The one word the queue knows a key by. The pair is hashed rather than
/// packed: a page's coordinate fills a word on its own, and a search's
/// subject is already a hash.
pub fn hash_of(name: DemandKey) -> u64 {
    let mut bytes = [0u8; 12];
    bytes[..4].copy_from_slice(&(name.category as u32).to_le_bytes());
    bytes[4..].copy_from_slice(&name.subject.to_le_bytes());
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, &bytes);
    std::hash::Hasher::finish(&h)
}

static SCHED: Global<Scheduler> = Global::new();

fn sched() -> &'static mut Scheduler {
    SCHED.get(|| Scheduler {
        queue: IoQueue::new(),
        credits: 0,
        categories: [None; CATEGORIES],
        category_count: 0,
        names: [(0, DemandKey::new(Category::Terrain, 0)); NAMES],
        name_count: 0,
    })
}

fn category_id(s: &Scheduler, category: Category) -> Option<u32> {
    s.categories[..s.category_count].iter().position(|c| *c == Some(category)).map(|id| id as u32)
}

fn category_of(s: &mut Scheduler, category: Category) -> u32 {
    if let Some(id) = category_id(s, category) {
        return id;
    }
    assert!(s.category_count < CATEGORIES, "more than {CATEGORIES} fetch categories");
    s.categories[s.category_count] = Some(category);
    s.category_count += 1;
    s.category_count as u32 - 1
}

fn key_of(s: &mut Scheduler, name: DemandKey) -> Option<u64> {
    let key = hash_of(name);
    if let Some((_, held)) = s.names[..s.name_count].iter().find(|(k, _)| *k == key) {
        return (*held == name).then_some(key);
    }
    if s.name_count == NAMES {
        return None;
    }
    s.names[s.name_count] = (key, name);
    s.name_count += 1;
    Some(key)
}

/// What a hash the queue handed back names, or None where nothing is held
/// under it.
pub fn demand_of(key: u64) -> Option<DemandKey> {
    let s = sched();
    s.names[..s.name_count].iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// The hash a key is held under, only when it is that key's.
fn held_key(s: &Scheduler, name: DemandKey) -> Option<u64> {
    let key = hash_of(name);
    s.names[..s.name_count].iter().any(|(k, n)| *k == key && *n == name).then_some(key)
}

fn forget_names(s: &mut Scheduler) {
    let mut kept = 0;
    for i in 0..s.name_count {
        let (key, _) = s.names[i];
        if s.queue.knows(key) || s.queue.aborted().contains(&key) {
            s.names.swap(kept, i);
            kept += 1;
        }
    }
    s.name_count = kept;
}

/// How many fetches may be in flight at once, as the central scheduler
/// grants, up to what the rows keep room for.
pub fn set_fetch_credits(credits: u32) {
    sched().credits = credits.min(MAX_CREDITS);
}

pub fn fetch_credits() -> u32 {
    sched().credits
}

/// Everything the scheduler holds, for the ledger's resources line.
pub fn fetch_counts() -> FetchCounts {
    sched().queue.counts()
}

/// Queued + in-flight demands in one category — map_status reads this.
pub fn fetch_pending_count(category: Category) -> u32 {
    let s = sched();
    category_id(s, category).map_or(0, |id| s.queue.pending_count(id))
}

/// The shell's report that a started demand settled: its outcome, whatever
/// split its run measured, and why it failed.
pub fn fetch_settle(key: DemandKey, outcome: Outcome, phases: Option<Phases>, fail: Failure) {
    let s = sched();
    // The slot may already be gone: a reset aborts and forgets in one step.
    let Some(settled) = held_key(s, key).and_then(|k| s.queue.settle(k)) else { return };
    ledger_record(TaskRecord {
        key,
        priority: settled.demand.priority,
        enqueue_ms: settled.enqueue_ms,
        start_ms: Some(settled.start_ms),
        end_ms: settled.end_ms,
        transport_ms: 0.0,
        phases,
        outcome,
        fail,
    });
    forget_names(s);
}

/// Runs the pump, within the credits granted: the turn's end, once every
/// category has synced.
pub fn fetch_pump() {
    let s = sched();
    s.queue.pump(s.credits);
}

/// Each demand the pump started since the last take, for the shell to run.
pub fn fetch_take_started(mut each: impl FnMut(&Demand)) {
    let s = sched();
    let started: Vec<(u64, f64)> = s.queue.started().iter().map(|d| (d.key, d.priority)).collect();
    for (key, priority) in started {
        if let Some(name) = demand_of(key) {
            each(&Demand { key: name, priority });
        }
    }
    sched().queue.clear_started();
}

/// Each key yield aborted since the last take, for the shell to cancel; each
/// is settled by the shell as Aborted.
pub fn fetch_take_aborts(mut each: impl FnMut(DemandKey)) {
    let s = sched();
    let aborted: Vec<u64> = s.queue.aborted().to_vec();
    for key in aborted {
        if let Some(name) = demand_of(key) {
            each(name);
        }
    }
    let s = sched();
    s.queue.clear_aborted();
    forget_names(s);
}

/// Replaces one category's wishlist. Demands for other categories are
/// untouched. New keys are wanted, keys already wanted pick up the new
/// priority, keys in flight are left running. Anything in the category that
/// isn't in `wanted` stops being wanted here and now, and whether anything is
/// told to stop is `fetch_pump`'s question, not this one. How many of
/// `wanted` found no room: those are not asked for until the wishlist is
/// given again.
pub fn fetch_sync(category: Category, wanted: &[Demand]) -> u32 {
    let s = sched();
    let id = category_of(s, category);
    let mut queued = [Queued::default(); HELD];
    let mut n = 0;
    for d in wanted.iter().take(HELD) {
        let Some(key) = key_of(s, d.key) else { continue };
        queued[n] = Queued {
            key,
            category: category_of(s, d.key.category),
            priority: d.priority,
        };
        n += 1;
    }
    let refused = s.queue.sync(id, &queued[..n]) + (wanted.len() - n) as u32;
    forget_names(s);
    refused
}

/// Whether a demand with this key is in flight right now. Read-only, for a
/// caller deciding whether to keep asking for something it has already asked
/// for (see terrain/tile_fetcher.rs's in-flight hysteresis).
pub fn fetch_running(key: DemandKey) -> bool {
    let s = sched();
    held_key(s, key).is_some_and(|k| s.queue.running(k))
}

/// Test-only: abort everything in flight and forget the queue, so each test
/// starts against an empty scheduler.
pub fn scheduler_reset() {
    sched().queue.reset();
}

/// A page's demand key: the stream, and the page's own coordinate.
pub fn page_key(category: Category, page_idx: IVec) -> DemandKey {
    DemandKey::new(category, page_idx)
}

#[cfg(test)]
#[path = "scheduler.test.rs"]
mod test;
