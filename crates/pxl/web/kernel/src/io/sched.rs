// pxl-kernel/io/sched.rs

//! The kernel's one I/O authority: what is wanted, what is out, and what may
//! be tried again.
//!
//! Three lifetimes, three tables, and nothing here keeps a state that is not
//! one of them:
//!
//! - **desired** — what a producer says it needs *now*. Replaced whole, per
//!   producer, on every turn that producer speaks.
//! - **attempt** — what the host was told to run and has not reported.
//! - **backoff** — when an identity whose attempt failed may be tried again.
//!
//! **A completion ends an attempt and touches neither of the others.** A
//! demand is not answered by being tried: a producer stops wanting something
//! by not saying it again, which it decides from its own store. That is the
//! whole of why nothing above needs to remember what it asked for last turn,
//! why a failure cannot make a producer re-ask every frame, and why a
//! producer never has to ask what is on the wire.
//!
//! The two tables are sized apart on purpose. What is wanted is bounded by
//! the producers; what is out is bounded by the credits. A demand that is
//! current can always be said, whatever old work is still draining.
//!
//! What order to run them in is not decided here — `io/policy.rs` projects
//! each producer's own facts to a `Rank`, and this compares ranks. Where two
//! ranks are equal the order is by category and then by subject: a tie-break
//! that means nothing and is the same every run.

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Phases};
use crate::sched::ledger::record::{ledger_record, Failure, Outcome, TaskRecord};

/// Fetches out at once at most, whatever the host grants.
pub const MAX_CREDITS: u32 = 16;

/// Where a producer's fact lands on the one scale this compares. `class` is
/// the grounded part — a lower class is work the viewer is more waiting on —
/// and `distance` orders within it. Both are `io/policy.rs`'s to assign; a
/// producer never sees this type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rank {
    pub class: u8,
    pub distance: f64,
}

impl Rank {}

/// One thing a producer wants. `subject` is the producer's own name for it.
#[derive(Clone, Copy, Debug)]
struct Wanted {
    category: Category,
    subject: u64,
    rank: Rank,
    /// When it was first wanted, kept across re-declarations so the ledger
    /// measures the wait rather than the last turn.
    enqueue_ms: f64,
}

/// One attempt the host was told to run.
#[derive(Clone, Copy, Debug)]
struct Attempt {
    category: Category,
    subject: u64,
    rank: Rank,
    enqueue_ms: f64,
    start_ms: f64,
    /// Told to stop. Still out, still spending its credit: a stop is a credit
    /// on its way back and not the end of anything.
    stopping: bool,
}

/// When a failed identity may be tried again.
#[derive(Clone, Copy, Debug)]
struct Backoff {
    category: Category,
    subject: u64,
    until_ms: f64,
}

#[derive(Default)]
struct Sched {
    desired: Vec<Wanted>,
    /// By slot. A slot's index and its generation are the key the host holds,
    /// so nothing has to keep a table of names: a key is only good while the
    /// attempt it names is out.
    attempt: Vec<Option<Attempt>>,
    generation: Vec<u32>,
    backoff: Vec<Backoff>,
    credits: u32,
    started: Vec<(u64, Category, u64)>,
    stops: Vec<u64>,
}

static SCHED: Global<Sched> = Global::new();

fn sched() -> &'static mut Sched {
    SCHED.get(|| Sched {
        attempt: vec![None; MAX_CREDITS as usize],
        generation: vec![0; MAX_CREDITS as usize],
        ..Sched::default()
    })
}

const fn key_of(slot: usize, generation: u32) -> u64 {
    (slot as u64) << 32 | generation as u64
}

fn slot_of(key: u64) -> Option<usize> {
    let (slot, generation) = ((key >> 32) as usize, key as u32);
    let s = sched();
    (s.generation.get(slot) == Some(&generation) && s.attempt.get(slot).is_some_and(Option::is_some)).then_some(slot)
}

/// Replaces one producer's desired set. Others are untouched.
///
/// Re-declaring the same set changes nothing observable, so a producer says
/// what it wants every turn and never has to notice whether that changed.
///
/// In place: a subject still wanted keeps its entry and when it was first
/// asked for, one no longer wanted leaves, one new joins — so a producer
/// saying the same thing again allocates nothing.
pub fn declare(category: Category, wanted: &[(u64, Rank)]) {
    let s = sched();
    s.desired.retain_mut(|w| {
        if w.category != category {
            return true;
        }
        match wanted.iter().find(|(k, _)| *k == w.subject) {
            Some(&(_, rank)) => {
                w.rank = rank;
                true
            }
            None => false,
        }
    });
    let at = now();
    for &(subject, rank) in wanted {
        if !s.desired.iter().any(|w| w.category == category && w.subject == subject) {
            s.desired.push(Wanted {
                category,
                subject,
                rank,
                enqueue_ms: at,
            });
        }
    }
}

pub fn set_credits(credits: u32) {
    sched().credits = credits.min(MAX_CREDITS);
}

pub fn credits() -> u32 {
    sched().credits
}

/// What is waiting and what is out, for a report.
pub fn counts() -> (u32, u32) {
    let s = sched();
    let out = s.attempt.iter().flatten().count() as u32;
    let waiting = s.desired.iter().filter(|w| !attempting(s, w.category, w.subject)).count() as u32;
    (waiting, out)
}

/// Wanted or out, of one category — for a report, never for deciding what to
/// want.
pub fn pending_count(category: Category) -> u32 {
    let s = sched();
    let wanted = s.desired.iter().filter(|w| w.category == category).count();
    let out = s
        .attempt
        .iter()
        .flatten()
        .filter(|a| a.category == category && !s.desired.iter().any(|w| w.category == a.category && w.subject == a.subject))
        .count();
    (wanted + out) as u32
}

fn attempting(s: &Sched, category: Category, subject: u64) -> bool {
    s.attempt.iter().flatten().any(|a| a.category == category && a.subject == subject)
}

fn held_back(s: &Sched, category: Category, subject: u64, at: f64) -> bool {
    s.backoff.iter().any(|b| b.category == category && b.subject == subject && at < b.until_ms)
}

/// Starts what is wanted and not out, best first, and where there is nothing
/// left to start with, takes a credit from work that ranks worse.
///
/// One pump a turn, after every producer has spoken: with a pump per
/// declaration whichever spoke first would fill every slot before another had
/// said anything, and rank could not order what had already gone.
pub fn pump() {
    let s = sched();
    let at = now();
    s.backoff.retain(|b| at < b.until_ms);

    // Best first, and where two rank the same, the same order every run.
    let mut ready: Vec<usize> = (0..s.desired.len())
        .filter(|&i| {
            let w = s.desired[i];
            !attempting(s, w.category, w.subject) && !held_back(s, w.category, w.subject, at)
        })
        .collect();
    ready.sort_by(|&a, &b| {
        let (x, y) = (s.desired[a], s.desired[b]);
        x.rank
            .class
            .cmp(&y.rank.class)
            .then(x.rank.distance.total_cmp(&y.rank.distance))
            // Equal rank is equal rank: what follows means nothing and is
            // only here so that two runs of the same turn start the same two.
            .then((x.category as u8, x.subject).cmp(&(y.category as u8, y.subject)))
    });

    let mut waiting = Vec::new();
    for i in ready {
        let w = s.desired[i];
        match free_slot(s) {
            Some(slot) => start(s, slot, w, at),
            None => waiting.push(w),
        }
    }
    preempt(s, &waiting);
}

fn free_slot(s: &Sched) -> Option<usize> {
    (0..s.credits as usize).find(|&k| s.attempt[k].is_none())
}

fn start(s: &mut Sched, slot: usize, w: Wanted, at: f64) {
    s.attempt[slot] = Some(Attempt {
        category: w.category,
        subject: w.subject,
        rank: w.rank,
        enqueue_ms: w.enqueue_ms,
        start_ms: at,
        stopping: false,
    });
    s.started.push((key_of(slot, s.generation[slot]), w.category, w.subject));
}

/// Takes a credit for what is wanted now from what is not, and only then from
/// work that ranks enough worse to be worth restarting.
///
/// A stop is logical: the attempt is still out and still spending its credit
/// until the host says it is over, and what it brings back is adopted like
/// anything else. The demand under a stopped attempt is not withdrawn — it is
/// still desired, and the next pump starts it again.
fn preempt(s: &mut Sched, waiting: &[Wanted]) {
    for w in waiting {
        // One already told to stop is a credit on its way back and counts
        // against the shortfall, so a turn does not tell a second.
        if s.attempt.iter().flatten().any(|a| a.stopping) {
            return;
        }
        let obsolete = |a: &Attempt| !s.desired.iter().any(|d| d.category == a.category && d.subject == a.subject);
        let victim = (0..s.attempt.len()).filter(|&k| s.attempt[k].is_some_and(|a| !a.stopping)).max_by(|&a, &b| {
            let (x, y) = (s.attempt[a].expect("some"), s.attempt[b].expect("some"));
            obsolete(&x).cmp(&obsolete(&y)).then(x.rank.class.cmp(&y.rank.class)).then(x.rank.distance.total_cmp(&y.rank.distance))
        });
        let Some(slot) = victim else { return };
        let a = s.attempt[slot].expect("some");
        // A whole class better, and nothing finer. Distance says which of two
        // alike to start, not that one running is worth restarting; taking a
        // credit costs everything the attempt has already spent, and only a
        // class says the waiting work is a different kind of wanted.
        if !obsolete(&a) && w.rank.class >= a.rank.class {
            return;
        }
        s.attempt[slot] = Some(Attempt { stopping: true, ..a });
        s.stops.push(key_of(slot, s.generation[slot]));
    }
}

/// Each attempt the pump started since the last take, for the host to run.
pub fn take_started(mut each: impl FnMut(u64, Category, u64)) {
    for (key, category, subject) in core::mem::take(&mut sched().started) {
        each(key, category, subject);
    }
}

/// Each attempt told to stop since the last take. Each is still settled by
/// the host like any other.
pub fn take_stops(mut each: impl FnMut(u64)) {
    for key in core::mem::take(&mut sched().stops) {
        each(key);
    }
}

/// What a key names, while its attempt is out.
pub fn attempt_of(key: u64) -> Option<(Category, u64)> {
    let slot = slot_of(key)?;
    sched().attempt[slot].map(|a| (a.category, a.subject))
}

/// The host's report that an attempt is over. The demand is left alone: a
/// producer stops wanting something by not saying it again.
///
/// `retry_in_ms` is how long this identity waits before another attempt —
/// none where the attempt answered, since an answer is not something to try
/// again, and the producer has already been given it.
pub fn settle(key: u64, outcome: Outcome, phases: Option<Phases>, fail: Failure, retry_in_ms: Option<f64>) {
    let Some(slot) = slot_of(key) else { return };
    let a = sched().attempt[slot].expect("a slot a key names");
    let s = sched();
    s.attempt[slot] = None;
    s.generation[slot] = s.generation[slot].wrapping_add(1);
    if let Some(wait) = retry_in_ms {
        let until_ms = now() + wait;
        match s.backoff.iter_mut().find(|b| b.category == a.category && b.subject == a.subject) {
            Some(b) => b.until_ms = until_ms,
            None => s.backoff.push(Backoff {
                category: a.category,
                subject: a.subject,
                until_ms,
            }),
        }
    }
    ledger_record(TaskRecord {
        key: crate::io::demand::DemandKey::new(a.category, a.subject),
        priority: a.rank.class as f64 + a.rank.distance,
        enqueue_ms: a.enqueue_ms,
        start_ms: Some(a.start_ms),
        end_ms: now(),
        transport_ms: 0.0,
        phases,
        outcome,
        fail,
    });
}

/// Stops everything out and forgets every table: the module is being reset
/// under a host that will not answer what it was told to run.
pub fn reset() {
    let s = sched();
    for slot in 0..s.attempt.len() {
        if s.attempt[slot].is_some() {
            s.attempt[slot] = None;
            s.generation[slot] = s.generation[slot].wrapping_add(1);
        }
    }
    s.desired.clear();
    s.backoff.clear();
    s.started.clear();
    s.stops.clear();
}

#[cfg(test)]
#[path = "sched.test.rs"]
mod test;
