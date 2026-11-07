// pxl-kernel/sys/io/request.rs

//! The kernel's request scheduler: what is wanted (`wants`, a run a
//! stream, replaced whole each turn) and what is out (`attempt`). Ordering, intake and
//! retry waits are the `Serve` policy's. What starts, stops and ends is
//! kept, in order, for the turn to take (`take_event`).
//!
//! An answer ends its want; a failure ends only its attempt. A failure of the
//! object drops its want; a body refused room, by the heap or the body
//! slots, holds its want until what refused it gives some back; one of the
//! network or the server holds back every start, then lets one probe out
//! until something answers, and the server's past its last wait gives the
//! reads up. A running attempt is never preempted.
//!
//! Invariants: no more attempts than credits and no want past its stream's
//! bound; ties broken by stream then id so the order is total; a free
//! credit and a ready want never both outlast a pump; an attempt stops only
//! when nobody wants it any more.

use core::cmp::Ordering;
use std::collections::VecDeque;

use crate::sys::io::demand::{Order, Phases, Source, Stream};
use crate::sys::io::failure::{Ended, Failure};
use crate::sys::io::policy::Policy;
use crate::sys::io::room::Refusal;
use crate::sys::load::LOAD_FILES;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_kit::math::rng::derive_seed_u64;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::param::{BODY_SLOTS, MAX_CREDITS};
use crate::sys::sched::edge;

/// Ordered by `class`, then `distance`, lower first, then `detail`, higher first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rank {
    pub class: u8,
    pub dist: f64,
    pub detail: u8,
}

pub trait Serve {
    /// Asked again every pump: the order is the view's now.
    fn rank(&self, stream: Stream, id: u64, order: Order, enqueue_ms: f64) -> Rank;

    fn intake(&self) -> u32;

    fn fault(&self, failure: Failure) -> Fault;

    /// The wait after the server's `failures`th failure in a row, none once
    /// it is given up; `seed` differs per failure.
    fn backoff(&self, failures: u32, seed: u64) -> Option<f64>;
}

/// Whose a failure is, which decides what waits on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The object's own: asking again brings the same.
    Object,
    /// No way to the server: waited on, never given up.
    Network,
    /// The server's: counted toward giving up.
    Server,
}

/// How many of a stream's wants are held at once. A layer's tiles: at most
/// its tile cache's slots, since it asks only for what it can hold; the
/// objects' shards, at most those the view asks for at once; each as the
/// layer says (`open`). An asset: one a slot, the catalog and the sphere.
pub type Bounds = [usize; Stream::ALL.len()];

fn bounds_of(frames: impl Fn(Stream) -> u32) -> Bounds {
    Stream::ALL.map(|c| match c.row().source {
        Source::Load => LOAD_FILES,
        Source::Bake(_) | Source::Objects => frames(c) as usize,
    })
}

/// A want, in its stream's run (`RequestScheduler::wants`).
#[derive(Clone, Copy, Debug)]
struct Want {
    id: u64,
    order: Order,
    /// First wanted, kept across re-declarations so the profiler measures the real wait.
    enqueue_ms: f64,
    wait: Wait,
}

impl Want {
    const NONE: Want = Want {
        id: 0,
        order: Order::LAST,
        enqueue_ms: 0.0,
        wait: Wait::Ready,
    };
}

/// What a want's own failure holds it back on; a hold on every start is the
/// scheduler's (`held_until`).
#[derive(Clone, Copy, Debug)]
enum Wait {
    Ready,
    /// Its body was refused room, the heap's or a body slot's: until that
    /// gives some back.
    Room(Refusal),
    /// Its object failed: for as long as it is wanted.
    Object,
}

impl Wait {
    fn ready(self) -> bool {
        match self {
            Wait::Ready => true,
            Wait::Room(refused) => refused.over(),
            Wait::Object => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Attempt {
    pub stream: Stream,
    pub id: u64,
    pub rank: Rank,
    pub enqueue_ms: f64,
    pub start_ms: f64,
}

/// What became of an attempt, kept for the turn to take (`take_event`).
pub enum Event {
    /// Started on its credit, for its reader to open.
    Start(Credit, Stream, u64),
    /// Stopped, nobody wanting it: its key, for its reader to close.
    Stop(u64),
    /// Ended, when, as what, and the timings its run reported.
    End(Attempt, f64, Ended, Option<Phases>),
}

#[derive(Clone, Copy)]
struct Ready {
    rank: Rank,
    stream: Stream,
    id: u64,
}

impl Ready {
    fn cmp(&self, other: &Ready) -> Ordering {
        self.rank
            .class
            .cmp(&other.rank.class)
            .then(self.rank.dist.total_cmp(&other.rank.dist))
            .then(other.rank.detail.cmp(&self.rank.detail))
            // Meaningless tie-break, so two runs of the same turn start the same work.
            .then((self.stream as u8, self.id).cmp(&(other.stream as u8, other.id)))
    }
}

pub struct RequestScheduler<S> {
    bounds: Bounds,
    serve: S,
    /// What is wanted: a stream's a run in a room its bound's wide, the
    /// streams' rooms in order (`first`), each run by id and as long as
    /// `run_lens` says.
    wants: Box<[Want]>,
    run_lens: [usize; Stream::ALL.len()],
    /// Where `declare` lays a stream's run before the run takes its place:
    /// as wide as the widest room.
    run_stage: Box<[Want]>,
    /// A slot's index and generation are the key the runtime holds; a key is valid
    /// only while its attempt is out.
    attempt: [Option<Attempt>; MAX_CREDITS as usize],
    generation: [u32; MAX_CREDITS as usize],
    credits: u32,
    /// The most credits ever granted.
    most_credits: u32,
    /// Oldest first.
    events: VecDeque<Event>,
    ready: Vec<(Ready, usize)>,
    /// Why reads fail, since the last that came to an answer.
    failing: Option<Errno>,
    /// The server's failures since the last answer, one an outage's wait:
    /// what was already out when one was counted fails with it.
    failures: u32,
    struck_ms: f64,
    /// No attempt starts before it.
    held_until: f64,
    /// The server failed past its last wait; only a reset ends it.
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
        let want_cap: usize = bounds.iter().sum();
        RequestScheduler {
            bounds,
            serve,
            wants: vec![Want::NONE; want_cap].into_boxed_slice(),
            run_lens: [0; Stream::ALL.len()],
            run_stage: vec![Want::NONE; bounds.iter().copied().max().unwrap_or(0)]
                .into_boxed_slice(),
            attempt: [None; MAX_CREDITS as usize],
            generation: [0; MAX_CREDITS as usize],
            credits: 0,
            most_credits: 0,
            events: VecDeque::new(),
            ready: Vec::with_capacity(want_cap),
            failing: None,
            failures: 0,
            struck_ms: f64::NEG_INFINITY,
            held_until: f64::NEG_INFINITY,
            given_up: false,
        }
    }

    /// Where stream `stream`'s room starts in `wants`: past every room
    /// before it.
    fn first(&self, stream: Stream) -> usize {
        self.bounds[..stream as usize].iter().sum()
    }

    fn run(&self, stream: Stream) -> &[Want] {
        let first = self.first(stream);
        &self.wants[first..first + self.run_lens[stream as usize]]
    }

    fn find(&self, stream: Stream, id: u64) -> Option<usize> {
        self.run(stream)
            .binary_search_by_key(&id, |w| w.id)
            .ok()
            .map(|k| self.first(stream) + k)
    }

    /// The want at `i` of `stream`'s run gone, the rest of the run closing up.
    fn drop_want(&mut self, stream: Stream, i: usize) {
        let end = self.first(stream) + self.run_lens[stream as usize];
        self.wants.copy_within(i + 1..end, i);
        self.run_lens[stream as usize] -= 1;
    }

    fn slot_of(&self, key: u64) -> Option<usize> {
        let (slot, generation) = ((key >> 32) as usize, key as u32);
        (self.generation.get(slot) == Some(&generation)
            && self.attempt.get(slot).is_some_and(Option::is_some))
        .then_some(slot)
    }

    fn attempting(&self, stream: Stream, id: u64) -> bool {
        self.attempt
            .iter()
            .flatten()
            .any(|a| a.stream == stream && a.id == id)
    }

    /// Replaces one stream's wants with `wanted`, by id: one walk of
    /// it beside the run held. Re-declaring the same set changes nothing, so
    /// producers say everything every turn. A kept id keeps its enqueue
    /// time and retry wait; a repeated id is one want, the later order.
    pub fn declare(&mut self, stream: Stream, wanted: &[(u64, Order)], at: f64) {
        xpute_kit::ensure!(
            wanted.windows(2).all(|p| p[0].0 <= p[1].0),
            ENOTRECOVERABLE,
            stream as u32
        );
        let (first, bound) = (self.first(stream), self.bounds[stream as usize]);
        let run = &self.wants[first..first + self.run_lens[stream as usize]];
        let (mut k, mut n) = (0, 0);
        for (j, &(id, order)) in wanted.iter().enumerate() {
            if wanted.get(j + 1).is_some_and(|w| w.0 == id) {
                continue;
            }
            while run.get(k).is_some_and(|w| w.id < id) {
                k += 1;
            }
            let w = match run.get(k) {
                Some(&w) if w.id == id => Want { order, ..w },
                _ => Want {
                    id,
                    order,
                    enqueue_ms: at,
                    wait: Wait::Ready,
                },
            };
            xpute_kit::ensure!(n < bound, ENOTRECOVERABLE, stream as u32);
            self.run_stage[n] = w;
            n += 1;
        }
        self.wants[first..first + n].copy_from_slice(&self.run_stage[..n]);
        self.run_lens[stream as usize] = n;
    }

    pub fn set_credits(&mut self, credits: u32) {
        self.credits = credits.min(MAX_CREDITS);
        self.most_credits = self.most_credits.max(self.credits);
    }

    #[cfg(test)]
    /// `stream`'s requests waiting for a credit and out on one. For a
    /// report only.
    pub fn stream_counts(&self, stream: Stream) -> (u32, u32) {
        let out = self
            .attempt
            .iter()
            .flatten()
            .filter(|a| a.stream == stream)
            .count() as u32;
        let waiting = self
            .run(stream)
            .iter()
            .filter(|w| !self.attempting(stream, w.id))
            .count() as u32;
        (waiting, out)
    }

    fn ready_of(&self, stream: Stream, w: &Want) -> Ready {
        Ready {
            rank: self.serve.rank(stream, w.id, w.order, w.enqueue_ms),
            stream,
            id: w.id,
        }
    }

    /// Stops what nobody wants, then starts ready wants in order up to the
    /// intake, into free credits only. One pump per turn after every producer
    /// has spoken, or the first producer would fill every slot.
    pub fn pump(&mut self, at: f64) {
        // Stop abandoned work in the turn it is abandoned: after a zoom out and
        // back in, credits must not sit on the zoom passed through.
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some_and(|a| self.find(a.stream, a.id).is_none()) {
                self.stop(slot, at);
            }
        }

        // A held server takes nothing; a failing one one probe at a time.
        if self.given_up || at < self.held_until {
            return;
        }
        let room = if self.failing.is_some() {
            1
        } else {
            self.credits as usize
        };
        // No credit free: nothing starts, so nothing is ranked.
        if self.attempt.iter().flatten().count() >= room || self.free_slot().is_none() {
            return;
        }
        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for s in Stream::ALL {
            let first = self.first(s);
            for (k, w) in self.run(s).iter().enumerate() {
                if w.wait.ready() && !self.attempting(s, w.id) {
                    ready.push((self.ready_of(s, w), first + k));
                }
            }
        }
        ready.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let intake = self.serve.intake().max(1) as usize;
        for &(r, i) in ready.iter().take(intake) {
            if self.attempt.iter().flatten().count() >= room {
                break;
            }
            let Some(slot) = self.free_slot() else { break };
            self.start(slot, r.stream, self.wants[i], r.rank, at);
        }
        self.ready = ready;
    }

    fn free_slot(&self) -> Option<usize> {
        (0..self.credits as usize).find(|&k| self.attempt[k].is_none())
    }

    fn start(&mut self, slot: usize, stream: Stream, w: Want, rank: Rank, at: f64) {
        self.attempt[slot] = Some(Attempt {
            stream,
            id: w.id,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        let key = key_of(slot, self.generation[slot]);
        self.events
            .push_back(Event::Start(Credit { key }, stream, w.id));
    }

    /// Ends the slot's attempt; its key is spent, so whatever the runtime still
    /// reports under it (an abort or a body that raced the stop) is dropped.
    fn end(&mut self, slot: usize) -> Attempt {
        let a = self.attempt[slot].take().or_bug(Errno::ENOTRECOVERABLE);
        self.generation[slot] = self.generation[slot].wrapping_add(1);
        a
    }

    fn stop(&mut self, slot: usize, at: f64) {
        let key = key_of(slot, self.generation[slot]);
        self.events.push_back(Event::Stop(key));
        let a = self.end(slot);
        self.events
            .push_back(Event::End(a, at, Ended::Aborted, None));
    }

    /// The oldest event not yet taken.
    pub fn take_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Whether `stream`'s want of `id` waits on its object, which
    /// failed: it is not asked again while it is wanted.
    pub fn has_object_failed(&self, stream: Stream, id: u64) -> bool {
        self.find(stream, id)
            .is_some_and(|i| matches!(self.wants[i].wait, Wait::Object))
    }

    pub fn attempt_of(&self, key: u64) -> Option<(Stream, u64)> {
        let slot = self.slot_of(key)?;
        self.attempt[slot].map(|a| (a.stream, a.id))
    }

    /// An answer ends the want; a failure holds it back.
    pub fn settle(&mut self, key: u64, ended: Ended, phases: Option<Phases>, at: f64) {
        let Some(slot) = self.slot_of(key) else {
            return;
        };
        let a = self.end(slot);
        let want = self.find(a.stream, a.id);
        match ended {
            Ended::Ok => {
                self.on_answered();
                if let Some(i) = want {
                    self.drop_want(a.stream, i);
                }
            }
            Ended::Failed(f) => self.on_failed(want, f, a.start_ms, at),
            // Still an answer from the server.
            Ended::Refused(refused) => {
                self.on_answered();
                if let Some(i) = want {
                    self.wants[i].wait = Wait::Room(refused);
                }
            }
            Ended::Aborted => {}
        }
        self.events.push_back(Event::End(a, at, ended, phases));
    }

    /// The server answered: whatever held it back is over.
    fn on_answered(&mut self) {
        self.failing = None;
        self.failures = 0;
        self.struck_ms = f64::NEG_INFINITY;
        self.held_until = f64::NEG_INFINITY;
    }

    fn on_failed(&mut self, want: Option<usize>, failure: Failure, started_ms: f64, at: f64) {
        let seed = derive_seed_u64(self.failures as u64, at.to_bits());
        let wait = match self.serve.fault(failure) {
            Fault::Object => Wait::Object,
            Fault::Network => {
                self.failing = Some(failure.errnum);
                self.held_until = at + self.serve.backoff(1, seed).unwrap_or(0.0);
                return;
            }
            Fault::Server => {
                self.failing = Some(failure.errnum);
                if started_ms <= self.struck_ms {
                    return;
                }
                self.struck_ms = at;
                self.failures += 1;
                match self.serve.backoff(self.failures, seed) {
                    Some(wait) => self.held_until = at + wait,
                    None => self.given_up = true,
                }
                return;
            }
        };
        // Still an answer from the server.
        self.on_answered();
        if let Some(i) = want {
            self.wants[i].wait = wait;
        }
    }

    /// The module is being reset under a runtime that will not answer what it was told to run.
    pub fn reset(&mut self) {
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some() {
                self.end(slot);
            }
        }
        self.run_lens = [0; Stream::ALL.len()];
        self.on_answered();
        self.given_up = false;
        // Its starts and stops were for the runtime gone; its ends stay the
        // profiler's.
        self.events.retain(|e| matches!(e, Event::End(..)));
    }
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

pub fn declare(stream: Stream, wanted: &[(u64, Order)]) {
    sched().declare(stream, wanted, now());
}

pub fn set_credits(credits: u32) {
    sched().set_credits(credits);
}

/// The body slots (`BODY_SLOTS`) a terrain packet may wait in past its
/// decode: less one for each of the most credits the runtime has granted,
/// one at least, as a body on each may still be out after a grant of fewer,
/// and one for each Earth tile its stream's bound holds, whose packet waits
/// for a frame (sys/io/body.rs). A packet kept within them never takes the
/// slot a read needs to arrive into.
pub fn spare_slots() -> u32 {
    let s = sched();
    let earth = s.bounds[Stream::earth as usize] as u32;
    BODY_SLOTS.saturating_sub(s.most_credits.max(1) + earth)
}

pub fn pump() {
    let s = sched();
    s.pump(now());
    // Nothing else need come to end a hold.
    if s.held_until.is_finite() && !s.given_up {
        edge::wake_at(s.held_until);
    }
}

/// One at a time, the scheduler let go between: a stop's reader settles
/// here.
pub fn take_event() -> Option<Event> {
    sched().take_event()
}

pub fn attempt_of(key: u64) -> Option<(Stream, u64)> {
    sched().attempt_of(key)
}

pub fn has_object_failed(stream: Stream, id: u64) -> bool {
    sched().has_object_failed(stream, id)
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

/// Whether the server failed past its last wait.
pub fn given_up() -> bool {
    sched().given_up
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
