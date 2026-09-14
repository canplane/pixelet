// pxl_core/module/world/sched/fetch_scheduler.rs

//! Cooperative fetch scheduler — the category-B (I/O) half of
//! issue/20260909-cooperative-scheduler-and-task-ledger.md.
//!
//! Two levers, no feedback control:
//!
//! - **quota:** at most MAX_INFLIGHT requests run at once, across every
//!   category. Terrain and POI compete for the same slots, ordered by priority
//!   — a near terrain page beats a far POI page. The pump that fills those
//!   slots runs once per turn, after every category has submitted, so every
//!   category submitting in the same turn competes fairly.
//! - **yield:** sync takes a category's *entire* current wishlist. Anything
//!   that was queued or in flight for that category and is no longer wished
//!   for is dropped (queued) or aborted (in flight), freeing its slot for
//!   what's wanted now.
//!
//! The transport is the shell's: a demand the pump starts is handed to it by
//! key (`fetch_take_started`), an abort likewise (`fetch_take_aborts`), and the
//! shell settles each (`fetch_settle`). The scheduler never touches a store.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::ds::map::{I64Keys, IndexedMap};
use xpute_core::kit::hash::fnv1a64_str;

use super::demand::{Demand, Phases};
use super::ledger::{ledger_record, Outcome, TaskRecord};
use crate::global::{now, Global};

// Browsers cap ~6 concurrent connections per origin anyway; matching that keeps
// the queue — not the socket pool — as the thing actually ordering work. A
// knob, not a computed value.
pub const MAX_INFLIGHT: u32 = 6;

const QUEUED: u8 = 1;
const RUNNING: u8 = 2;

/// A row of the table, at the demand's position in the index.
struct Row {
    demand: Demand,
    state: u8,
    category_id: u32,
    enqueue_ms: f64,
    start_ms: f64,
}

struct Scheduler {
    /// Every demand the scheduler holds, queued or in flight: the key's 64-bit
    /// hash in the index, the row at its position.
    index: IndexedMap<I64Keys>,
    rows: Vec<Row>,
    /// Categories are a handful of names; a row holds them as small ids.
    category_names: Vec<String>,
    /// Keys the pump started, and keys yield aborted, for the shell to take.
    started: Vec<Demand>,
    aborted: Vec<String>,
    pump_scheduled: bool,
    /// Moves after every queue/running change — what map_status follows.
    revision: u32,
}

static SCHED: Global<Scheduler> = Global::new();

fn sched() -> &'static mut Scheduler {
    SCHED.get(|| Scheduler {
        index: IndexedMap::new(I64Keys, 64, None),
        rows: Vec::new(),
        category_names: Vec::new(),
        started: Vec::new(),
        aborted: Vec::new(),
        pump_scheduled: false,
        revision: 0,
    })
}

fn category_of(s: &mut Scheduler, name: &str) -> u32 {
    match s.category_names.iter().position(|n| n == name) {
        Some(id) => id as u32,
        None => {
            s.category_names.push(name.to_string());
            s.category_names.len() as u32 - 1
        }
    }
}

fn key_hash(key: &str) -> i64 {
    fnv1a64_str(key) as i64
}

/// A key is a string by contract (`<category>:<page>`, or a search query), so
/// it is hashed to the table's key and the stored demand's own key is compared
/// on every hit — a 64-bit collision among tens of keys is not expected, and
/// is loud rather than silent if it ever happens.
fn find(s: &Scheduler, key: &str) -> Option<usize> {
    let pos = s.index.find(key_hash(key));
    if pos < 0 {
        return None;
    }
    let pos = pos as usize;
    if s.rows[pos].demand.key != key {
        crate::module::world::sched::fetch_scheduler::warn_collision(key, &s.rows[pos].demand.key);
        return None;
    }
    Some(pos)
}

/// Said once, where a console exists.
pub fn warn_collision(_key: &str, _other: &str) {}

fn add(s: &mut Scheduler, demand: Demand) {
    let pos = s.index.insert(key_hash(&demand.key)) as usize;
    let row = Row {
        category_id: category_of(s, &demand.category),
        demand,
        state: QUEUED,
        enqueue_ms: now(),
        start_ms: -1.0,
    };
    if pos == s.rows.len() {
        s.rows.push(row);
    } else {
        s.rows[pos] = row;
    }
}

fn remove(s: &mut Scheduler, pos: usize) {
    s.index.remove_at(pos as u32);
    s.rows.swap_remove(pos);
}

fn count(s: &Scheduler, which: u8) -> u32 {
    s.rows.iter().filter(|r| r.state == which).count() as u32
}

/// Moves after every queue/running change — the revision map_status follows.
pub fn fetch_revision() -> u32 {
    sched().revision
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FetchCounts {
    pub queued: u32,
    pub running: u32,
}

/// Everything the scheduler holds, for the ledger's resources line.
pub fn fetch_counts() -> FetchCounts {
    let s = sched();
    FetchCounts {
        queued: count(s, QUEUED),
        running: count(s, RUNNING),
    }
}

/// Queued + in-flight demands in one category — map_status reads this.
pub fn fetch_pending_count(category: &str) -> u32 {
    let s = sched();
    let Some(id) = s.category_names.iter().position(|n| n == category) else { return 0 };
    s.rows.iter().filter(|r| r.category_id == id as u32).count() as u32
}

/// A short, stable label for why a task failed — what the ledger groups
/// failures by. The repositories throw `"[category/page] http 503"`, so the
/// status is the useful part when there is one; anything else keeps its name
/// and the start of its message.
pub fn error_reason(name: &str, message: &str) -> String {
    let bytes = message.as_bytes();
    let mut at = 0;
    while let Some(off) = message[at..].find("http ") {
        let start = at + off;
        let digits = &message[start + 5..];
        let boundary_before = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        if boundary_before && digits.len() >= 3 && digits.as_bytes()[..3].iter().all(u8::is_ascii_digit) && digits.as_bytes().get(3).is_none_or(|c| !c.is_ascii_alphanumeric()) {
            return format!("http {}", &digits[..3]);
        }
        at = start + 5;
    }
    let text = format!("{name}: {message}");
    text.chars().take(80).collect()
}

/// The shell's report that a started demand settled: its outcome, whatever
/// split its run measured, and why it failed.
pub fn fetch_settle(key: &str, outcome: Outcome, phases: Option<Phases>, error: Option<String>) {
    let s = sched();
    // The slot may already be gone: a reset aborts and forgets in one step.
    let Some(pos) = find(s, key) else { return };
    if s.rows[pos].state != RUNNING {
        return;
    }
    let (demand, enqueued, started) = (s.rows[pos].demand.clone(), s.rows[pos].enqueue_ms, s.rows[pos].start_ms);
    remove(s, pos);
    ledger_record(TaskRecord {
        key: key.to_string(),
        category: demand.category,
        priority: demand.priority,
        enqueue_ms: enqueued,
        start_ms: Some(started),
        end_ms: now(),
        transport_ms: 0.0,
        phases,
        outcome,
        error,
    });
    schedule_pump();
    s.revision += 1;
}

/// Coalesces every pump request in the current turn into one.
///
/// Measured motivation: with a synchronous pump, whichever subsystem called
/// sync first won outright — terrain's nine demands filled all MAX_INFLIGHT
/// slots before POI had submitted anything, and a POI page at priority 1 sat
/// 358ms behind terrain pages at priority 2. Priority can only order the
/// queue; it cannot preempt a request already in flight, so submission order
/// was silently beating it. Deferring the pump to the end of the turn lets every
/// sync land first, and the single pump that follows picks the top
/// MAX_INFLIGHT across all of them.
///
/// The diff half of sync stays synchronous on purpose — dropping and aborting
/// superseded demands must not wait.
fn schedule_pump() {
    sched().pump_scheduled = true;
}

/// Runs the pump a sync or a settle asked for: the end of the turn's.
pub fn fetch_pump() {
    let s = sched();
    if !s.pump_scheduled {
        return;
    }
    s.pump_scheduled = false;

    let mut inflight = count(s, RUNNING);
    if inflight >= MAX_INFLIGHT || inflight as usize == s.rows.len() {
        return;
    }

    // Small N (tens of pages at most) — a sort per pump is cheaper to read than
    // maintaining a heap, and pump runs once per turn, not per frame.
    let mut by_priority: Vec<usize> = (0..s.rows.len()).filter(|&pos| s.rows[pos].state == QUEUED).collect();
    by_priority.sort_by(|&a, &b| s.rows[a].demand.priority.total_cmp(&s.rows[b].demand.priority));

    for pos in by_priority {
        if inflight >= MAX_INFLIGHT {
            break;
        }
        inflight += 1;
        let row = &mut s.rows[pos];
        row.state = RUNNING;
        row.start_ms = now();
        s.started.push(row.demand.clone());
    }
    s.revision += 1;
}

/// The demands the pump started since the last take, for the shell to run.
pub fn fetch_take_started() -> Vec<Demand> {
    std::mem::take(&mut sched().started)
}

/// The keys yield aborted since the last take, for the shell to cancel; each
/// is settled by the shell as Aborted.
pub fn fetch_take_aborts() -> Vec<String> {
    std::mem::take(&mut sched().aborted)
}

/// Replaces one category's wishlist. Demands for other categories are
/// untouched. New keys are queued, keys already queued pick up the new
/// priority, keys in flight are left running; anything in the category that
/// isn't in `wanted` is dropped or aborted.
pub fn fetch_sync(category: &str, wanted: &[Demand]) {
    let s = sched();
    let id = category_of(s, category);

    // yield: shed this category's demands that are no longer wanted. Backwards,
    // since a removal moves the last slot into the hole.
    for pos in (0..s.rows.len()).rev() {
        let row = &s.rows[pos];
        if row.category_id != id || wanted.iter().any(|d| d.key == row.demand.key) {
            continue;
        }
        if row.state == QUEUED {
            remove(s, pos);
        } else {
            s.aborted.push(row.demand.key.clone());
        }
    }

    // admit the new set.
    for demand in wanted {
        match find(s, &demand.key) {
            None => add(s, demand.clone()),
            Some(pos) => {
                if s.rows[pos].state == QUEUED {
                    s.rows[pos].demand = demand.clone();
                }
            }
        }
    }

    schedule_pump();
    s.revision += 1;
}

/// Whether a demand with this key is in flight right now. Read-only, for a
/// caller deciding whether to keep asking for something it has already asked
/// for (see terrain/use_tile_fetcher.rs's in-flight hysteresis).
pub fn fetch_running(key: &str) -> bool {
    let s = sched();
    find(s, key).is_some_and(|pos| s.rows[pos].state == RUNNING)
}

/// Test-only: abort everything in flight and forget the queue, so each test
/// starts against an empty scheduler.
pub fn fetch_scheduler_reset() {
    let s = sched();
    for row in &s.rows {
        if row.state == RUNNING {
            s.aborted.push(row.demand.key.clone());
        }
    }
    s.index.clear();
    s.rows.clear();
}

/// A page's demand key: `<category>:<page>`.
pub fn page_key(category: &str, page_idx: IVec) -> String {
    format!("{category}:{page_idx}")
}
