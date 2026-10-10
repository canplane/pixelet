// pxl-kernel/io/request.rs

//! The kernel's request scheduler: what is wanted (`desired`, replaced whole per
//! stream each turn) and what is out (`attempt`). Ordering, intake and
//! retry waits are the `Serve` policy's.
//!
//! An answer ends its want; a failure ends only its attempt and waits on the
//! want, or gives it up. A running attempt is never preempted.
//!
//! Invariants: no more attempts than credits and no want past its stream's
//! bound; ties broken by stream then subject so the order is total; a free
//! credit and a ready want never both outlast a pump; an attempt stops only
//! when nobody wants it any more.

use core::cmp::Ordering;

use crate::io::demand::{Ask, Phases, Stream};
use crate::io::failure::{Ended, Failure};
use crate::io::load::LOADS;
use crate::io::policy::Policy;
use crate::profiler::record::profiler_record;
use crate::profiler::task::TaskRow;
use std::collections::HashMap;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::base::off_of_slice;
use xpute_kit::math::rng::derive_seed_u64;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

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
    fn rank(&self, stream: Stream, subject: u64, ask: Ask, enqueue_ms: f64) -> Rank;

    fn intake(&self) -> u32;

    /// What a want does after its `failures`th failure; `seed` differs per want and per failure.
    fn retry(&self, failure: Failure, failures: u32, seed: u64) -> Retry;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Retry {
    After(f64),
    GiveUp,
}

/// How many of a stream's wants are held at once. A layer's tiles: at most
/// its residency's frames, since it asks only for what it can hold, which the
/// layer says (`open`). An asset: as a model and as an image.
pub type Bounds = [usize; Stream::ALL.len()];

fn bounds_of(frames: impl Fn(Stream) -> u32) -> Bounds {
    Stream::ALL.map(|c| match c {
        Stream::asset => 2 * LOADS,
        Stream::terrain | Stream::place | Stream::earth | Stream::division => frames(c) as usize,
    })
}

#[derive(Clone, Copy, Debug)]
struct Wanted {
    stream: Stream,
    subject: u64,
    ask: Ask,
    /// First wanted, kept across re-declarations so the profiler measures the real wait.
    enqueue_ms: f64,
    until_ms: f64,
    /// Since it was first wanted; kept across re-declarations, as the wait is.
    failures: u32,
}

#[derive(Clone, Copy, Debug)]
struct Attempt {
    stream: Stream,
    subject: u64,
    rank: Rank,
    enqueue_ms: f64,
    start_ms: f64,
}

#[derive(Clone, Copy)]
struct Placed {
    rank: Rank,
    stream: Stream,
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
            .then((self.stream as u8, self.subject).cmp(&(other.stream as u8, other.subject)))
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
    started: Vec<(u64, Stream, u64)>,
    stops: Vec<u64>,
    ready: Vec<(Placed, usize)>,
    /// Why reads fail, since the last that came to an answer.
    failing: Option<Errno>,
    /// A want failed past its retries; only a reset ends it.
    given_up: bool,
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
            failing: None,
            given_up: false,
        }
    }

    fn find(&self, stream: Stream, subject: u64) -> Option<usize> {
        self.desired.iter().position(|w| w.is_some_and(|w| w.stream == stream && w.subject == subject))
    }

    fn slot_of(&self, key: u64) -> Option<usize> {
        let (slot, generation) = ((key >> 32) as usize, key as u32);
        (self.generation.get(slot) == Some(&generation) && self.attempt.get(slot).is_some_and(Option::is_some)).then_some(slot)
    }

    fn attempting(&self, stream: Stream, subject: u64) -> bool {
        self.attempt.iter().flatten().any(|a| a.stream == stream && a.subject == subject)
    }

    /// Replaces one stream's wants. Re-declaring the same set changes
    /// nothing, so producers say everything every turn. A kept subject keeps its
    /// enqueue time and retry wait; a repeated subject is one want, the later ask.
    pub fn declare(&mut self, stream: Stream, wanted: &[(u64, Ask)], at: f64) {
        for entry in self.desired.iter_mut() {
            let Some(w) = entry else { continue };
            if w.stream != stream {
                continue;
            }
            match wanted.iter().rev().find(|(k, _)| *k == w.subject) {
                Some(&(_, ask)) => w.ask = ask,
                None => *entry = None,
            }
        }
        let mut held = self.desired.iter().flatten().filter(|w| w.stream == stream).count();
        for &(subject, ask) in wanted {
            if self.find(stream, subject).is_some() {
                continue;
            }
            xpute_kit::ensure!(held < self.bounds[stream as usize], ENOTRECOVERABLE, stream as u32);
            held += 1;
            let free = self.desired.iter().position(Option::is_none).or_bug(Errno::ENOTRECOVERABLE);
            self.desired[free] = Some(Wanted {
                stream,
                subject,
                ask,
                enqueue_ms: at,
                until_ms: f64::NEG_INFINITY,
                failures: 0,
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
        let waiting = self.desired.iter().flatten().filter(|w| !self.attempting(w.stream, w.subject)).count() as u32;
        (waiting, out)
    }

    /// For a report only, never for deciding what to want.
    pub fn pending_count(&self, stream: Stream) -> u32 {
        let wanted = self.desired.iter().flatten().filter(|w| w.stream == stream).count();
        let out = self.attempt.iter().flatten().filter(|a| a.stream == stream && self.find(a.stream, a.subject).is_none()).count();
        (wanted + out) as u32
    }

    fn placed(&self, w: &Wanted) -> Placed {
        Placed {
            rank: self.serve.rank(w.stream, w.subject, w.ask, w.enqueue_ms),
            stream: w.stream,
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
            if self.attempt[slot].is_some_and(|a| self.find(a.stream, a.subject).is_none()) {
                self.stop(slot, at);
            }
        }

        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for (i, w) in self.desired.iter().enumerate() {
            let Some(w) = w else { continue };
            if at >= w.until_ms && !self.attempting(w.stream, w.subject) {
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
            stream: w.stream,
            subject: w.subject,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        self.started.push((key_of(slot, self.generation[slot]), w.stream, w.subject));
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
        record(&a, at, Ended::Aborted, None);
    }

    pub fn take_started(&mut self, mut each: impl FnMut(Credit, Stream, u64)) {
        for (key, stream, subject) in self.started.drain(..) {
            each(Credit { key }, stream, subject);
        }
    }

    pub fn take_stops(&mut self, mut each: impl FnMut(u64)) {
        for key in self.stops.drain(..) {
            each(key);
        }
    }

    pub fn attempt_of(&self, key: u64) -> Option<(Stream, u64)> {
        let slot = self.slot_of(key)?;
        self.attempt[slot].map(|a| (a.stream, a.subject))
    }

    /// An answer ends the want; a failure holds it back.
    pub fn settle(&mut self, key: u64, ended: Ended, phases: Option<Phases>, at: f64) {
        let Some(slot) = self.slot_of(key) else { return };
        let a = self.end(slot);
        match ended {
            Ended::Ok => self.failing = None,
            // An absent file is an answer.
            Ended::Failed(f) if f.errno != Errno::ENOENT => self.failing = Some(f.errno),
            _ => {}
        }
        if let Some(i) = self.find(a.stream, a.subject) {
            match ended {
                Ended::Ok => self.desired[i] = None,
                Ended::Failed(f) => self.hold_back(i, f, at),
                Ended::Aborted => {}
            }
        }
        record(&a, at, ended, phases);
    }

    /// A failed want waits as the policy says, or is given up: never asked again.
    fn hold_back(&mut self, i: usize, failure: Failure, at: f64) {
        let Some(w) = &mut self.desired[i] else { return };
        w.failures += 1;
        let seed = derive_seed_u64(w.subject ^ ((w.stream as u64) << 56), at.to_bits());
        match self.serve.retry(failure, w.failures, seed) {
            Retry::After(wait) => w.until_ms = at + wait,
            Retry::GiveUp => {
                w.until_ms = f64::INFINITY;
                self.given_up = true;
            }
        }
    }

    /// The module is being reset under a host that will not answer what it was told to run.
    pub fn reset(&mut self) {
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some() {
                self.end(slot);
            }
        }
        self.desired.fill(None);
        self.failing = None;
        self.given_up = false;
        self.started.clear();
        self.stops.clear();
    }
}

fn record(a: &Attempt, at: f64, ended: Ended, phases: Option<Phases>) {
    profiler_record(|| {
        let mut row = TaskRow {
            stream: a.stream as u32 as f64,
            subject: a.subject,
            priority: a.rank.class as f64 + a.rank.distance,
            enqueue_ms: a.enqueue_ms,
            start_ms: a.start_ms,
            end_ms: at,
            outcome: ended.outcome() as u32 as f64,
            ..TaskRow::default()
        };
        if let Ended::Failed(fail) = ended {
            row.fail = f64::from(fail.errno as i32);
            row.detail = fail.detail as f64;
        }
        if let Some(at) = match ended {
            Ended::Failed(fail) => fail.at,
            Ended::Ok | Ended::Aborted => None,
        } {
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
pub fn open(frames: impl Fn(Stream) -> u32) {
    SCHED.set(RequestScheduler::new(Policy::standard(), bounds_of(frames)));
}

fn sched() -> &'static mut RequestScheduler<Policy> {
    // Asked before `open` is the roof wired wrong, never a state to wait out.
    SCHED.get(|| None.or_bug(Errno::ENOTRECOVERABLE))
}

pub fn declare(stream: Stream, wanted: &[(u64, Ask)]) {
    sched().declare(stream, wanted, now());
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

pub fn pending_count(stream: Stream) -> u32 {
    sched().pending_count(stream)
}

pub fn pump() {
    sched().pump(now());
}

pub fn take_started(each: impl FnMut(Credit, Stream, u64)) {
    sched().take_started(each);
}

pub fn take_stops(each: impl FnMut(u64)) {
    sched().take_stops(each);
}

pub fn attempt_of(key: u64) -> Option<(Stream, u64)> {
    sched().attempt_of(key)
}

pub fn settle(key: u64, ended: Ended, phases: Option<Phases>) {
    sched().settle(key, ended, phases, now());
}

pub fn reset() {
    sched().reset();
}

/// Why reads fail, where none has come to an answer since one did.
pub fn failing() -> Option<Errno> {
    sched().failing
}

/// Whether a want failed past its retries.
pub fn given_up() -> bool {
    sched().given_up
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
