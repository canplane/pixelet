// pxl-kernel/io/request.rs

//! The kernel's request scheduler: what is wanted (`desired`, replaced whole per
//! category each turn) and what is out (`attempt`). Ordering, intake and
//! retry waits are the `Serve` policy's.
//!
//! An answer ends its want; a failure ends only its attempt and waits on the
//! want. A running attempt is never preempted.
//!
//! Invariants: no more attempts than credits and no want past its category's
//! bound; ties broken by category then subject so the order is total; a free
//! credit and a ready want never both outlast a pump; an attempt stops only
//! when nobody wants it any more.

use core::cmp::Ordering;

use crate::io::demand::{Ask, Category, Phases};
use crate::io::failure::{Failure, Outcome};
use crate::io::load::LOADS;
use crate::io::policy::Policy;
use crate::profiler::record::profiler_record;
use crate::profiler::task::TaskRow;
use std::collections::HashMap;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::base::off_of_slice;

use crate::param::MAX_CREDITS;

/// Ordered by `class`, then `distance`, lower first, then `detail`, higher first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rank {
    pub class: u8,
    pub distance: f64,
    pub detail: u8,
}

pub trait Serve {
    /// Asked again every pump: the order is the view's now.
    fn rank(&self, category: Category, subject: u64, ask: Ask, enqueue_ms: f64) -> Rank;

    fn intake(&self) -> u32;

    fn retry_after(&self, outcome: Outcome) -> Option<f64>;
}

/// How many of a category's wants are held at once. A layer's tiles: at most
/// its residency's frames, since it asks only for what it can hold, which the
/// layer says (`open`). An asset: as a model and as an image. Streams of one: one.
pub type Bounds = [usize; Category::ALL.len()];

fn bounds_of(frames: impl Fn(Category) -> u32) -> Bounds {
    Category::ALL.map(|c| match c {
        Category::asset => 2 * LOADS,
        Category::geocode | Category::geocode_dict => 1,
        Category::terrain | Category::poi | Category::earth => frames(c) as usize,
    })
}

#[derive(Clone, Copy, Debug)]
struct Wanted {
    category: Category,
    subject: u64,
    ask: Ask,
    /// First wanted, kept across re-declarations so the profiler measures the real wait.
    enqueue_ms: f64,
    until_ms: f64,
}

#[derive(Clone, Copy, Debug)]
struct Attempt {
    category: Category,
    subject: u64,
    rank: Rank,
    enqueue_ms: f64,
    start_ms: f64,
}

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
            .then(other.rank.detail.cmp(&self.rank.detail))
            // Meaningless tie-break, so two runs of the same turn start the same work.
            .then((self.category as u8, self.subject).cmp(&(other.category as u8, other.subject)))
    }
}

pub struct RequestScheduler<S> {
    bounds: Bounds,
    serve: S,
    desired: Box<[Option<Wanted>]>,
    /// A slot's index and generation are the key the host holds; a key is valid
    /// only while its attempt is out.
    attempt: [Option<Attempt>; MAX_CREDITS as usize],
    generation: [u32; MAX_CREDITS as usize],
    credits: u32,
    started: Vec<(u64, Category, u64)>,
    stops: Vec<u64>,
    ready: Vec<(Placed, usize)>,
}

/// A started attempt's right to open a file: only the scheduler makes one, so
/// what is open at once is no more than its credits.
pub struct Credit {
    key: u64,
}

impl Credit {
    pub fn key(&self) -> u64 {
        self.key
    }
}

const fn key_of(slot: usize, generation: u32) -> u64 {
    (slot as u64) << 32 | generation as u64
}

impl<S: Serve> RequestScheduler<S> {
    pub fn new(serve: S, bounds: Bounds) -> RequestScheduler<S> {
        let desired: usize = bounds.iter().sum();
        RequestScheduler {
            bounds,
            serve,
            desired: vec![None; desired].into_boxed_slice(),
            attempt: [None; MAX_CREDITS as usize],
            generation: [0; MAX_CREDITS as usize],
            credits: 0,
            started: Vec::new(),
            stops: Vec::new(),
            ready: Vec::with_capacity(desired),
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

    /// Replaces one category's wants. Re-declaring the same set changes
    /// nothing, so producers say everything every turn. A kept subject keeps its
    /// enqueue time and retry wait; a repeated subject is one want, the later ask.
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
            xpute_core::ensure!(held < self.bounds[category as usize], ENOTRECOVERABLE, category as u32);
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

    pub fn counts(&self) -> (u32, u32) {
        let out = self.attempt.iter().flatten().count() as u32;
        let waiting = self.desired.iter().flatten().filter(|w| !self.attempting(w.category, w.subject)).count() as u32;
        (waiting, out)
    }

    /// For a report only, never for deciding what to want.
    pub fn pending_count(&self, category: Category) -> u32 {
        let wanted = self.desired.iter().flatten().filter(|w| w.category == category).count();
        let out = self.attempt.iter().flatten().filter(|a| a.category == category && self.find(a.category, a.subject).is_none()).count();
        (wanted + out) as u32
    }

    fn placed(&self, w: &Wanted) -> Placed {
        Placed {
            rank: self.serve.rank(w.category, w.subject, w.ask, w.enqueue_ms),
            category: w.category,
            subject: w.subject,
        }
    }

    /// Stops what nobody wants, then starts ready wants in order up to the
    /// intake, into free credits only. One pump per turn after every producer
    /// has spoken, or the first producer would fill every slot.
    pub fn pump(&mut self, at: f64) {
        // Stop abandoned work in the turn it is abandoned: after a zoom out and
        // back in, credits must not sit on the zoom passed through.
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

        let intake = self.serve.intake().max(1) as usize;
        for &(placed, i) in ready.iter().take(intake) {
            let Some(slot) = self.free_slot() else { break };
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

    /// Ends the slot's attempt; its key is spent, so whatever the host still
    /// reports under it (an abort or a body that raced the stop) is dropped.
    fn end(&mut self, slot: usize) -> Attempt {
        let a = self.attempt[slot].take().or_bug(Errno::ENOTRECOVERABLE);
        self.generation[slot] = self.generation[slot].wrapping_add(1);
        a
    }

    fn stop(&mut self, slot: usize, at: f64) {
        self.stops.push(key_of(slot, self.generation[slot]));
        let a = self.end(slot);
        record(&a, at, Outcome::aborted, None, Failure::default());
    }

    pub fn take_started(&mut self, mut each: impl FnMut(Credit, Category, u64)) {
        for (key, category, subject) in self.started.drain(..) {
            each(Credit { key }, category, subject);
        }
    }

    pub fn take_stops(&mut self, mut each: impl FnMut(u64)) {
        for key in self.stops.drain(..) {
            each(key);
        }
    }

    pub fn attempt_of(&self, key: u64) -> Option<(Category, u64)> {
        let slot = self.slot_of(key)?;
        self.attempt[slot].map(|a| (a.category, a.subject))
    }

    /// An answer ends the want; a failure holds it back.
    pub fn settle(&mut self, key: u64, outcome: Outcome, phases: Option<Phases>, fail: Failure, at: f64) {
        let Some(slot) = self.slot_of(key) else { return };
        let a = self.end(slot);
        if let Some(i) = self.find(a.category, a.subject) {
            if outcome == Outcome::ok {
                self.desired[i] = None;
            } else if let (Some(wait), Some(w)) = (self.serve.retry_after(outcome), &mut self.desired[i]) {
                w.until_ms = at + wait;
            }
        }
        record(&a, at, outcome, phases, fail);
    }

    /// The module is being reset under a host that will not answer what it was told to run.
    pub fn reset(&mut self) {
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some() {
                self.end(slot);
            }
        }
        self.desired.fill(None);
        self.started.clear();
        self.stops.clear();
    }
}

fn record(a: &Attempt, at: f64, outcome: Outcome, phases: Option<Phases>, fail: Failure) {
    profiler_record(|| {
        let mut row = TaskRow {
            category: a.category as u32 as f64,
            subject: a.subject,
            priority: a.rank.class as f64 + a.rank.distance,
            enqueue_ms: a.enqueue_ms,
            start_ms: a.start_ms,
            end_ms: at,
            outcome: outcome as u32 as f64,
            fail: fail.errno as f64,
            detail: fail.detail as f64,
            ..TaskRow::default()
        };
        if let Some(at) = fail.at {
            row.at_file = off_of_slice(file_in_memory(at.file()).as_bytes()) as f64;
            row.at_file_bytes = at.file().len() as f64;
            row.at_line = at.line() as f64;
        }
        for (k, (name, value)) in phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name as u32 as f64;
            row.phase_values[k] = value;
            row.phase_count = (k + 1) as f64;
        }
        row
    });
}

static FILES: Global<HashMap<usize, Box<str>>> = Global::new();

/// A location's file name lies outside kernel memory on a device, so rows
/// name a heap copy; the names are few and kept for good.
fn file_in_memory(file: &'static str) -> &'static str {
    FILES.get(HashMap::new).entry(file.as_ptr().addr()).or_insert_with(|| file.into())
}

static SCHED: Global<RequestScheduler<Policy>> = Global::new();

/// Opened once the kernel is laid out, with each layer's frames.
pub fn open(frames: impl Fn(Category) -> u32) {
    SCHED.set(RequestScheduler::new(Policy::standard(), bounds_of(frames)));
}

fn sched() -> &'static mut RequestScheduler<Policy> {
    // Asked before `open` is the roof wired wrong, never a state to wait out.
    SCHED.get(|| None.or_bug(Errno::ENOTRECOVERABLE))
}

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

pub fn pump() {
    sched().pump(now());
}

pub fn take_started(each: impl FnMut(Credit, Category, u64)) {
    sched().take_started(each);
}

pub fn take_stops(each: impl FnMut(u64)) {
    sched().take_stops(each);
}

pub fn attempt_of(key: u64) -> Option<(Category, u64)> {
    sched().attempt_of(key)
}

pub fn settle(key: u64, outcome: Outcome, phases: Option<Phases>, fail: Failure) {
    sched().settle(key, outcome, phases, fail, now());
}

pub fn reset() {
    sched().reset();
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
