// pxl_core/module/world/sched/fetch_scheduler.rs

//! The world's fetches, on the kernel's I/O queue (xpute_core::sched::io):
//! what a demand is called — `<category>:<page>`, or what stands for a search
//! query — and the ledger's record of each, over a queue that knows keys as
//! numbers.
//!
//! The quota is the central scheduler's: it grants the credits each turn
//! (`set_fetch_credits`), and the pump at the turn's end starts no more
//! than that many at once. What the pump started and what yield aborted are
//! the shell's to take (`fetch_take_started`, `fetch_take_aborts`); the shell
//! runs each and settles it (`fetch_settle`). The scheduler never touches a
//! store.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::hash::fnv1a64_str;
pub use xpute_core::sched::io::IoCounts as FetchCounts;
use xpute_core::sched::io::{Demand as Queued, IoQueue};

use super::demand::{demand_key, Demand, DemandKey, ErrorText, Phases};
use super::ledger::{ledger_record, Outcome, TaskRecord};
use crate::global::Global;

/// Demands held at once: the pages near the view in each category, and a
/// search. Past it a demand is refused and asked for again.
const HELD: usize = 128;

/// Names held at once: each key the queue holds, and each it aborted and has
/// not handed out — at most one of each per slot.
const NAMES: usize = HELD * 2;

/// Categories: a handful of names.
const CATEGORIES: usize = 16;

struct Scheduler {
    queue: IoQueue<HELD>,
    credits: u32,
    /// The queue holds a category as its index here.
    categories: [&'static str; CATEGORIES],
    category_count: usize,
    /// The name of every key the queue holds or has aborted, by the name's
    /// 64-bit hash. A second name under a hash already taken is refused
    /// rather than confused with the first — not expected among tens of keys.
    names: [(u64, DemandKey); NAMES],
    name_count: usize,
}

static SCHED: Global<Scheduler> = Global::new();

fn sched() -> &'static mut Scheduler {
    SCHED.get(|| Scheduler {
        queue: IoQueue::new(),
        credits: 0,
        categories: [""; CATEGORIES],
        category_count: 0,
        names: core::array::from_fn(|_| (0, DemandKey::new())),
        name_count: 0,
    })
}

fn category_id(s: &Scheduler, name: &str) -> Option<u32> {
    s.categories[..s.category_count].iter().position(|n| *n == name).map(|id| id as u32)
}

fn category_of(s: &mut Scheduler, name: &'static str) -> u32 {
    if let Some(id) = category_id(s, name) {
        return id;
    }
    assert!(s.category_count < CATEGORIES, "more than {CATEGORIES} fetch categories");
    s.categories[s.category_count] = name;
    s.category_count += 1;
    s.category_count as u32 - 1
}

fn key_of(s: &mut Scheduler, name: &DemandKey) -> Option<u64> {
    let key = fnv1a64_str(name);
    if let Some((_, held)) = s.names[..s.name_count].iter().find(|(k, _)| *k == key) {
        return (held == name).then_some(key);
    }
    if s.name_count == NAMES {
        return None;
    }
    s.names[s.name_count] = (key, name.clone());
    s.name_count += 1;
    Some(key)
}

fn name_of(s: &Scheduler, key: u64) -> DemandKey {
    s.names[..s.name_count].iter().find(|(k, _)| *k == key).map(|(_, n)| n.clone()).unwrap_or_default()
}

/// The key a name is held under, only when it is that name's.
fn held_key(s: &Scheduler, name: &str) -> Option<u64> {
    let key = fnv1a64_str(name);
    s.names[..s.name_count].iter().any(|(k, n)| *k == key && n.as_str() == name).then_some(key)
}

fn forget_names(s: &mut Scheduler) {
    let mut kept = 0;
    for i in 0..s.name_count {
        let (key, _) = s.names[i];
        if s.queue.holds(key) || s.queue.aborted().contains(&key) {
            s.names.swap(kept, i);
            kept += 1;
        }
    }
    s.name_count = kept;
}

/// How many fetches may be in flight at once, as the central scheduler grants.
pub fn set_fetch_credits(credits: u32) {
    sched().credits = credits;
}

pub fn fetch_credits() -> u32 {
    sched().credits
}

/// Moves after every queue/running change — the revision map_status follows.
pub fn fetch_revision() -> u32 {
    sched().queue.revision()
}

/// Everything the scheduler holds, for the ledger's resources line.
pub fn fetch_counts() -> FetchCounts {
    sched().queue.counts()
}

/// Queued + in-flight demands in one category — map_status reads this.
pub fn fetch_pending_count(category: &str) -> u32 {
    let s = sched();
    category_id(s, category).map_or(0, |id| s.queue.pending_count(id))
}

/// A short, stable label for why a task failed — what the ledger groups
/// failures by. The repositories throw `"[category/page] http 503"`, so the
/// status is the useful part when there is one; anything else keeps its name
/// and the start of its message.
pub fn error_reason(name: &str, message: &str) -> ErrorText {
    let bytes = message.as_bytes();
    let mut out = ErrorText::new();
    let mut at = 0;
    while let Some(off) = message[at..].find("http ") {
        let start = at + off;
        let digits = &message[start + 5..];
        // A word character, as the boundary this reads for was written: an
        // underscore is one, so `x_http 503` names no status.
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let boundary_before = start == 0 || !word(bytes[start - 1]);
        if boundary_before && digits.len() >= 3 && digits.as_bytes()[..3].iter().all(u8::is_ascii_digit) && digits.as_bytes().get(3).is_none_or(|c| !word(*c)) {
            return message[start..start + 8].to_string();
        }
        at = start + 5;
    }
    // Eighty as the shell counts a string's length, so one reason reads the
    // same on both sides of the ring; a character is never cut in half.
    let mut units = 0;
    for c in name.chars().chain(": ".chars()).chain(message.chars()) {
        units += c.len_utf16();
        if units > 80 {
            break;
        }
        out.push(c);
    }
    out
}

/// The shell's report that a started demand settled: its outcome, whatever
/// split its run measured, and why it failed.
pub fn fetch_settle(key: &str, outcome: Outcome, phases: Option<Phases>, error: Option<ErrorText>) {
    let s = sched();
    // The slot may already be gone: a reset aborts and forgets in one step.
    let Some(settled) = held_key(s, key).and_then(|k| s.queue.settle(k)) else { return };
    ledger_record(TaskRecord {
        key: DemandKey::from(key),
        category: s.categories[settled.demand.category as usize],
        priority: settled.demand.priority,
        enqueue_ms: settled.enqueue_ms,
        start_ms: Some(settled.start_ms),
        end_ms: settled.end_ms,
        transport_ms: 0.0,
        phases,
        outcome,
        error,
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
    for d in s.queue.started() {
        each(&Demand {
            key: name_of(s, d.key),
            category: s.categories[d.category as usize],
            priority: d.priority,
        });
    }
    s.queue.clear_started();
}

/// Each key yield aborted since the last take, for the shell to cancel; each
/// is settled by the shell as Aborted.
pub fn fetch_take_aborts(mut each: impl FnMut(&DemandKey)) {
    let s = sched();
    for &key in s.queue.aborted() {
        each(&name_of(s, key));
    }
    s.queue.clear_aborted();
    forget_names(s);
}

/// Replaces one category's wishlist. Demands for other categories are
/// untouched. New keys are queued, keys already queued pick up the new
/// priority, keys in flight are left running; anything in the category that
/// isn't in `wanted` is dropped or aborted.
pub fn fetch_sync(category: &'static str, wanted: &[Demand]) {
    let s = sched();
    let id = category_of(s, category);
    let mut queued = [Queued::default(); HELD];
    let mut n = 0;
    for d in wanted.iter().take(HELD) {
        let Some(key) = key_of(s, &d.key) else { continue };
        queued[n] = Queued {
            key,
            category: category_of(s, d.category),
            priority: d.priority,
        };
        n += 1;
    }
    s.queue.sync(id, &queued[..n]);
    forget_names(s);
}

/// Whether a demand with this key is in flight right now. Read-only, for a
/// caller deciding whether to keep asking for something it has already asked
/// for (see terrain/use_tile_fetcher.rs's in-flight hysteresis).
pub fn fetch_running(key: &str) -> bool {
    let s = sched();
    held_key(s, key).is_some_and(|k| s.queue.running(k))
}

/// Test-only: abort everything in flight and forget the queue, so each test
/// starts against an empty scheduler.
pub fn fetch_scheduler_reset() {
    sched().queue.reset();
}

/// A page's demand key: `<category>:<page>`.
pub fn page_key(category: &str, page_idx: IVec) -> DemandKey {
    demand_key(category, page_idx)
}

#[cfg(test)]
#[path = "fetch_scheduler.test.rs"]
mod test;
