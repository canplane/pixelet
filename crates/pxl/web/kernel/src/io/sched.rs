// pxl-kernel/io/sched.rs

//! The kernel's one I/O authority: what is wanted, what is out, and what may
//! be tried again. This is the mechanism; which of the waiting runs first,
//! whether one running is stopped for it, how many a turn takes on and how
//! long a failure waits are a serve policy's (`Serve`, io/policy.rs), and
//! nothing here decides any of them.
//!
//! Two tables, and nothing here keeps a state that is not one of them:
//!
//! - **desired** — what a producer says it needs *now*, and when a failed
//!   attempt at it may be tried again. Replaced whole, per category, on
//!   every turn that category speaks.
//! - **attempt** — what the host was told to run, while it is wanted and
//!   the host has not reported.
//!
//! **A completion ends an attempt and touches nothing else.** A demand is
//! not answered by being tried: a producer stops wanting something by not
//! saying it again, which it decides from its own store. That is the whole of
//! why nothing above needs to remember what it asked for last turn, why a
//! failure cannot make a producer re-ask every frame, and why a producer
//! never has to ask what is on the wire.
//!
//! A failure's wait lives on the want it failed for, so a want withdrawn
//! forgets it and one said again later is tried at once. The other shape, a
//! table of waits kept past their wants, is bounded by nothing but how many
//! distinct things failed in a wait's span; this one is bounded by the wants.
//!
//! The two tables are sized apart on purpose, and both at compile time. What
//! is wanted is bounded by what each category can ask for at once (`bound`);
//! what is out is bounded by the credits. A demand that is current can always
//! be said, whatever old work is still draining.
//!
//! What the mechanism keeps, whatever the policy:
//!
//! - **Safety.** No more attempts out than the credits, and no want past its
//!   category's bound.
//! - **A total order.** The policy ranks; where two ranks are equal the
//!   order is by category and then by subject, a tie-break that means
//!   nothing and is the same every run, so the policy's order is always a
//!   total one.
//! - **Work conservation.** A free credit and a want ready for it do not
//!   both outlast a pump.
//! - **Liveness at rest.** A running attempt is stopped for a waiting one
//!   only where the policy asks it and the waiting one stands strictly
//!   before it in that total order. While what is wanted and its ranks hold
//!   still, every stop moves the running set strictly earlier in a finite
//!   order, so stops end, the credits drain, and every want still standing is
//!   started. A policy whose ranks move while nothing else does can still
//!   starve a want; not starving one is the policy's side of the contract.

use core::cmp::Ordering;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Ask, Category, DemandKey, Phases};
use crate::io::load::LOADS;
use crate::io::policy::Policy;
use crate::sched::profiler::record::{profiler_record, Failure, Outcome, TaskRecord};
use crate::world::paging::FRAMES;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use crate::param::MAX_CREDITS;

/// Where a serve policy places one want on the one scale this compares:
/// `class` first, lower first, then `distance`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rank {
    pub class: u8,
    pub distance: f64,
}

/// How the waiting work is served: its order, whether a running attempt is
/// stopped for a waiting one, how many attempts a pump starts, and how long
/// a failure waits. Swapping it changes latency and nothing a want's answer
/// depends on.
pub trait Serve {
    /// Where a want stands, from what its producer said of it and when it
    /// was first wanted. Asked again every pump: the order is the view's
    /// now, not when the want was made.
    fn rank(&self, category: Category, subject: u64, ask: Ask, enqueue_ms: f64) -> Rank;

    /// Whether a want ranked `waiting` is worth the credit of an attempt
    /// ranked `running`. Asked only where `waiting` already stands strictly
    /// before `running`.
    fn preempts(&self, waiting: Rank, running: Rank) -> bool;

    /// The most attempts one pump starts; at least one is started where a
    /// credit is free.
    fn intake(&self) -> u32;

    /// How long an identity waits after an attempt ended with `outcome`
    /// before another; none where it may be tried at once.
    fn retry_after(&self, outcome: Outcome) -> Option<f64>;
}

/// The most one category can want at once: a layer's pages at most the
/// frames it pages them into (world/paging.rs `FRAMES`), since what it asks
/// for is what it can hold; an asset as a model and as an image; one of each
/// stream of one.
const fn bound(category: Category) -> usize {
    match category {
        Category::terrain | Category::poi => FRAMES as usize,
        Category::asset => 2 * LOADS,
        Category::ocean | Category::geocode | Category::geocode_dict => 1,
    }
}

/// Every category's bound together: the desired table's size.
const DESIRED: usize = bound(Category::terrain) + bound(Category::poi) + bound(Category::asset) + 3;

/// One thing a producer wants. `subject` is the producer's own name for it.
#[derive(Clone, Copy, Debug)]
struct Wanted {
    category: Category,
    subject: u64,
    ask: Ask,
    /// When it was first wanted, kept across re-declarations so the profiler
    /// measures the wait rather than the last turn.
    enqueue_ms: f64,
    /// When a failed attempt at it may be tried again.
    until_ms: f64,
}

/// One attempt the host was told to run.
#[derive(Clone, Copy, Debug)]
struct Attempt {
    category: Category,
    subject: u64,
    rank: Rank,
    enqueue_ms: f64,
    start_ms: f64,
}

/// A want placed in the total order: the policy's rank, then category and
/// subject.
#[derive(Clone, Copy)]
struct Placed {
    rank: Rank,
    category: Category,
    subject: u64,
}

impl Placed {
    fn cmp(&self, other: &Placed) -> Ordering {
        self.rank
            .class
            .cmp(&other.rank.class)
            .then(self.rank.distance.total_cmp(&other.rank.distance))
            // Equal rank is equal rank: what follows means nothing and is
            // only here so that two runs of the same turn start the same two.
            .then((self.category as u8, self.subject).cmp(&(other.category as u8, other.subject)))
    }
}

pub struct Ledger<S> {
    serve: S,
    desired: Box<[Option<Wanted>]>,
    /// By slot. A slot's index and its generation are the key the host holds,
    /// so nothing has to keep a table of names: a key is only good while the
    /// attempt it names is out.
    attempt: [Option<Attempt>; MAX_CREDITS as usize],
    generation: [u32; MAX_CREDITS as usize],
    credits: u32,
    /// What the pump started and stopped since the host last took them,
    /// drained every turn.
    started: Vec<(u64, Category, u64)>,
    stops: Vec<u64>,
    /// The pump's own room for ordering what is ready, as big as the desired
    /// table and never grown.
    ready: Vec<(Placed, usize)>,
}

const fn key_of(slot: usize, generation: u32) -> u64 {
    (slot as u64) << 32 | generation as u64
}

impl<S: Serve> Ledger<S> {
    pub fn new(serve: S) -> Ledger<S> {
        Ledger {
            serve,
            desired: vec![None; DESIRED].into_boxed_slice(),
            attempt: [None; MAX_CREDITS as usize],
            generation: [0; MAX_CREDITS as usize],
            credits: 0,
            started: Vec::new(),
            stops: Vec::new(),
            ready: Vec::with_capacity(DESIRED),
        }
    }

    fn find(&self, category: Category, subject: u64) -> Option<usize> {
        self.desired.iter().position(|w| w.is_some_and(|w| w.category == category && w.subject == subject))
    }

    fn slot_of(&self, key: u64) -> Option<usize> {
        let (slot, generation) = ((key >> 32) as usize, key as u32);
        (self.generation.get(slot) == Some(&generation) && self.attempt.get(slot).is_some_and(Option::is_some)).then_some(slot)
    }

    fn attempting(&self, category: Category, subject: u64) -> bool {
        self.attempt.iter().flatten().any(|a| a.category == category && a.subject == subject)
    }

    /// Replaces one category's desired set. Others are untouched.
    ///
    /// Re-declaring the same set changes nothing observable, so a producer
    /// says what it wants every turn and never has to notice whether that
    /// changed. A subject still wanted keeps its entry, when it was first
    /// asked for and when it may be tried again; one no longer wanted leaves;
    /// one new joins. A subject said twice is one want, the later ask.
    pub fn declare(&mut self, category: Category, wanted: &[(u64, Ask)], at: f64) {
        for entry in self.desired.iter_mut() {
            let Some(w) = entry else { continue };
            if w.category != category {
                continue;
            }
            match wanted.iter().rev().find(|(k, _)| *k == w.subject) {
                Some(&(_, ask)) => w.ask = ask,
                None => *entry = None,
            }
        }
        let mut held = self.desired.iter().flatten().filter(|w| w.category == category).count();
        for &(subject, ask) in wanted {
            if self.find(category, subject).is_some() {
                continue;
            }
            // A category asking past its bound is a producer asking for more
            // than it can hold, which the bound is derived from.
            xpute_core::ensure!(held < bound(category), ENOTRECOVERABLE, category as u32);
            held += 1;
            let free = self.desired.iter().position(Option::is_none).or_bug(Errno::ENOTRECOVERABLE);
            self.desired[free] = Some(Wanted {
                category,
                subject,
                ask,
                enqueue_ms: at,
                until_ms: f64::NEG_INFINITY,
            });
        }
    }

    pub fn set_credits(&mut self, credits: u32) {
        self.credits = credits.min(MAX_CREDITS);
    }

    pub fn credits(&self) -> u32 {
        self.credits
    }

    /// What is waiting and what is out, for a report.
    pub fn counts(&self) -> (u32, u32) {
        let out = self.attempt.iter().flatten().count() as u32;
        let waiting = self.desired.iter().flatten().filter(|w| !self.attempting(w.category, w.subject)).count() as u32;
        (waiting, out)
    }

    /// Wanted or out, of one category — for a report, never for deciding
    /// what to want.
    pub fn pending_count(&self, category: Category) -> u32 {
        let wanted = self.desired.iter().flatten().filter(|w| w.category == category).count();
        let out = self.attempt.iter().flatten().filter(|a| a.category == category && self.find(a.category, a.subject).is_none()).count();
        (wanted + out) as u32
    }

    /// A want's place in the total order, as the policy ranks it now.
    fn placed(&self, w: &Wanted) -> Placed {
        Placed {
            rank: self.serve.rank(w.category, w.subject, w.ask, w.enqueue_ms),
            category: w.category,
            subject: w.subject,
        }
    }

    /// Stops what no producer wants any more, then starts what is wanted and
    /// not out, in the policy's order, up to its intake, and where there is
    /// no free credit, takes one from the attempt that stands last where the
    /// policy asks it and the waiting want stands strictly before.
    ///
    /// One pump a turn, after every producer has spoken: with a pump per
    /// declaration whichever spoke first would fill every slot before another
    /// had said anything, and rank could not order what had already gone.
    pub fn pump(&mut self, at: f64) {
        // What the camera has left is stopped in the turn it leaves, every one
        // of it: a zoom out and straight back in must not leave the credits on
        // the level it passed through while the level wanted now waits for
        // them one round trip at a time.
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some_and(|a| self.find(a.category, a.subject).is_none()) {
                self.stop(slot, at);
            }
        }

        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for (i, w) in self.desired.iter().enumerate() {
            let Some(w) = w else { continue };
            if at >= w.until_ms && !self.attempting(w.category, w.subject) {
                ready.push((self.placed(w), i));
            }
        }
        ready.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        // Each turn of the loop starts one or ends it, so the intake is a
        // count of the ready taken.
        let intake = self.serve.intake().max(1) as usize;
        for &(placed, i) in ready.iter().take(intake) {
            // In order, so where nothing makes room for this one nothing
            // makes room for what follows it.
            let Some(slot) = self.free_slot().or_else(|| self.preempt(placed, at)) else { break };
            let w = self.desired[i].or_bug(Errno::ENOTRECOVERABLE);
            self.start(slot, w, placed.rank, at);
        }
        self.ready = ready;
    }

    fn free_slot(&self) -> Option<usize> {
        (0..self.credits as usize).find(|&k| self.attempt[k].is_none())
    }

    fn start(&mut self, slot: usize, w: Wanted, rank: Rank, at: f64) {
        self.attempt[slot] = Some(Attempt {
            category: w.category,
            subject: w.subject,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        self.started.push((key_of(slot, self.generation[slot]), w.category, w.subject));
    }

    /// Takes a credit for `waiting` from the attempt that stands last, where
    /// `waiting` stands strictly before it and the policy asks it, and says
    /// which slot it freed. The demand under the attempt stopped is not
    /// withdrawn: it is still desired, and a later pump starts it again.
    fn preempt(&mut self, waiting: Placed, at: f64) -> Option<usize> {
        let running = |k: usize| {
            let a = self.attempt[k]?;
            let w = self.desired[self.find(a.category, a.subject)?]?;
            Some((self.placed(&w), k))
        };
        let (last, slot) = (0..self.credits as usize).filter_map(running).max_by(|a, b| a.0.cmp(&b.0))?;
        if waiting.cmp(&last).is_ge() || !self.serve.preempts(waiting.rank, last.rank) {
            return None;
        }
        self.stop(slot, at);
        Some(slot)
    }

    /// Ends an attempt from this side: the host is told to stop it, and the
    /// credit is free at once. The key is spent with it, so whatever the host
    /// still reports under it — an abort, or a body that landed as the stop
    /// went out — finds no attempt and is let go (io/fetch.rs). A body that
    /// came as the camera left is lost that way; one it comes back for is
    /// asked again.
    fn stop(&mut self, slot: usize, at: f64) {
        let a = self.attempt[slot].take().or_bug(Errno::ENOTRECOVERABLE);
        self.stops.push(key_of(slot, self.generation[slot]));
        self.generation[slot] = self.generation[slot].wrapping_add(1);
        profiler_record(TaskRecord {
            key: DemandKey::new(a.category, a.subject),
            priority: a.rank.class as f64 + a.rank.distance,
            enqueue_ms: a.enqueue_ms,
            start_ms: Some(a.start_ms),
            end_ms: at,
            phases: None,
            outcome: Outcome::aborted,
            fail: Failure::default(),
        });
    }

    /// Each attempt the pump started since the last take, for the host to
    /// run.
    pub fn take_started(&mut self, mut each: impl FnMut(u64, Category, u64)) {
        for (key, category, subject) in self.started.drain(..) {
            each(key, category, subject);
        }
    }

    /// Each attempt stopped since the last take, for the host to abort. Its
    /// key is spent: what the host reports under it afterwards names nothing.
    pub fn take_stops(&mut self, mut each: impl FnMut(u64)) {
        for key in self.stops.drain(..) {
            each(key);
        }
    }

    /// What a key names, while its attempt is out.
    pub fn attempt_of(&self, key: u64) -> Option<(Category, u64)> {
        let slot = self.slot_of(key)?;
        self.attempt[slot].map(|a| (a.category, a.subject))
    }

    /// The host's report that an attempt is over. The demand is left alone:
    /// a producer stops wanting something by not saying it again. Where the
    /// policy says the outcome waits, the want, if it still stands, is held
    /// back that long.
    pub fn settle(&mut self, key: u64, outcome: Outcome, phases: Option<Phases>, fail: Failure, at: f64) {
        let Some(slot) = self.slot_of(key) else { return };
        let a = self.attempt[slot].take().or_bug(Errno::ENOTRECOVERABLE);
        self.generation[slot] = self.generation[slot].wrapping_add(1);
        if let (Some(wait), Some(i)) = (self.serve.retry_after(outcome), self.find(a.category, a.subject)) {
            if let Some(w) = &mut self.desired[i] {
                w.until_ms = at + wait;
            }
        }
        profiler_record(TaskRecord {
            key: DemandKey::new(a.category, a.subject),
            priority: a.rank.class as f64 + a.rank.distance,
            enqueue_ms: a.enqueue_ms,
            start_ms: Some(a.start_ms),
            end_ms: at,
            phases,
            outcome,
            fail,
        });
    }

    /// Stops everything out and forgets every table: the module is being
    /// reset under a host that will not answer what it was told to run.
    pub fn reset(&mut self) {
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].take().is_some() {
                self.generation[slot] = self.generation[slot].wrapping_add(1);
            }
        }
        self.desired.fill(None);
        self.started.clear();
        self.stops.clear();
    }
}

// ---- the kernel's ledger ----

static SCHED: Global<Ledger<Policy>> = Global::new();

fn sched() -> &'static mut Ledger<Policy> {
    SCHED.get(|| Ledger::new(Policy::standard()))
}

/// `Ledger::declare` on the kernel's ledger, now.
pub fn declare(category: Category, wanted: &[(u64, Ask)]) {
    sched().declare(category, wanted, now());
}

pub fn set_credits(credits: u32) {
    sched().set_credits(credits);
}

pub fn credits() -> u32 {
    sched().credits()
}

pub fn counts() -> (u32, u32) {
    sched().counts()
}

pub fn pending_count(category: Category) -> u32 {
    sched().pending_count(category)
}

/// `Ledger::pump` on the kernel's ledger, now.
pub fn pump() {
    sched().pump(now());
}

pub fn take_started(each: impl FnMut(u64, Category, u64)) {
    sched().take_started(each);
}

pub fn take_stops(each: impl FnMut(u64)) {
    sched().take_stops(each);
}

pub fn attempt_of(key: u64) -> Option<(Category, u64)> {
    sched().attempt_of(key)
}

/// `Ledger::settle` on the kernel's ledger, now.
pub fn settle(key: u64, outcome: Outcome, phases: Option<Phases>, fail: Failure) {
    sched().settle(key, outcome, phases, fail, now());
}

pub fn reset() {
    sched().reset();
}

#[cfg(test)]
#[path = "sched.test.rs"]
mod test;
