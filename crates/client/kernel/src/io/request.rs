// pxl-kernel/io/request.rs

//! The kernel's request scheduler: what is wanted (`desired`, a run a
//! stream, replaced whole each turn) and what is out (`attempt`). Ordering, intake and
//! retry waits are the `Serve` policy's.
//!
//! An answer ends its want; a failure ends only its attempt. A failure of the
//! object drops its want; a body refused room, by the heap or the body
//! slots, holds its want until what refused it gives some back; one of the
//! network or the server holds back every start, then lets one probe out
//! until something answers, and the server's past its last wait gives the
//! reads up. A running attempt is never preempted.
//!
//! Invariants: no more attempts than credits and no want past its stream's
//! bound; ties broken by stream then subject so the order is total; a free
//! credit and a ready want never both outlast a pump; an attempt stops only
//! when nobody wants it any more.

use core::cmp::Ordering;

use crate::asset::LOAD_FILES;
use crate::io::demand::{Ask, Phases, Stream};
use crate::io::failure::{Ended, Failure};
use crate::io::host::Refused;
use crate::io::policy::Policy;
use crate::profiler::record::profiler_record;
use crate::profiler::task::TaskRow;
use fnv::FnvHashMap;
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

    fn blame(&self, failure: Failure) -> Blame;

    /// The wait after the server's `strikes`th failure in a row, none once
    /// it is given up; `seed` differs per failure.
    fn backoff(&self, strikes: u32, seed: u64) -> Option<f64>;
}

/// Whose a failure is, which decides what waits on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blame {
    /// The object's own: asking again brings the same.
    Object,
    /// No way to the server: waited on, never given up.
    Network,
    /// The server's: counted toward giving up.
    Server,
}

/// How many of a stream's wants are held at once. A layer's tiles: at most
/// its residency's frames, since it asks only for what it can hold, which the
/// layer says (`open`). An asset: as a model and as an image.
pub type Bounds = [usize; Stream::ALL.len()];

fn bounds_of(frames: impl Fn(Stream) -> u32) -> Bounds {
    Stream::ALL.map(|c| match c {
        Stream::asset => LOAD_FILES,
        Stream::terrain | Stream::place | Stream::earth | Stream::division => frames(c) as usize,
    })
}

/// A want, in its stream's run (`RequestScheduler::desired`).
#[derive(Clone, Copy, Debug)]
struct Wanted {
    subject: u64,
    ask: Ask,
    /// First wanted, kept across re-declarations so the profiler measures the real wait.
    enqueue_ms: f64,
    wait: Wait,
}

impl Wanted {
    const NONE: Wanted = Wanted {
        subject: 0,
        ask: Ask::PLAIN,
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
    Room(Refused),
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
    /// What is wanted: a stream's a run in a room its bound's wide, the
    /// streams' rooms in order (`first`), each run by subject and as long as
    /// `held` says.
    desired: Box<[Wanted]>,
    held: [usize; Stream::ALL.len()],
    /// Where `declare` lays a stream's run before the run takes its place:
    /// as wide as the widest room.
    merged: Box<[Wanted]>,
    /// A slot's index and generation are the key the host holds; a key is valid
    /// only while its attempt is out.
    attempt: [Option<Attempt>; MAX_CREDITS as usize],
    generation: [u32; MAX_CREDITS as usize],
    credits: u32,
    /// The most credits ever granted.
    most_credits: u32,
    started: Vec<(u64, Stream, u64)>,
    stops: Vec<u64>,
    ready: Vec<(Placed, usize)>,
    /// Why reads fail, since the last that came to an answer.
    failing: Option<Errno>,
    /// The server's failures since the last answer, one an outage's wait:
    /// what was already out when one was counted fails with it.
    strikes: u32,
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
        let desired: usize = bounds.iter().sum();
        RequestScheduler {
            bounds,
            serve,
            desired: vec![Wanted::NONE; desired].into_boxed_slice(),
            held: [0; Stream::ALL.len()],
            merged: vec![Wanted::NONE; bounds.iter().copied().max().unwrap_or(0)].into_boxed_slice(),
            attempt: [None; MAX_CREDITS as usize],
            generation: [0; MAX_CREDITS as usize],
            credits: 0,
            most_credits: 0,
            started: Vec::new(),
            stops: Vec::new(),
            ready: Vec::with_capacity(desired),
            failing: None,
            strikes: 0,
            struck_ms: f64::NEG_INFINITY,
            held_until: f64::NEG_INFINITY,
            given_up: false,
        }
    }

    /// Where stream `stream`'s room starts in `desired`: past every room
    /// before it.
    fn first(&self, stream: Stream) -> usize {
        self.bounds[..stream as usize].iter().sum()
    }

    fn run(&self, stream: Stream) -> &[Wanted] {
        let first = self.first(stream);
        &self.desired[first..first + self.held[stream as usize]]
    }

    fn find(&self, stream: Stream, subject: u64) -> Option<usize> {
        self.run(stream).binary_search_by_key(&subject, |w| w.subject).ok().map(|k| self.first(stream) + k)
    }

    /// The want at `i` of `stream`'s run gone, the rest of the run closing up.
    fn drop_want(&mut self, stream: Stream, i: usize) {
        let end = self.first(stream) + self.held[stream as usize];
        self.desired.copy_within(i + 1..end, i);
        self.held[stream as usize] -= 1;
    }

    fn slot_of(&self, key: u64) -> Option<usize> {
        let (slot, generation) = ((key >> 32) as usize, key as u32);
        (self.generation.get(slot) == Some(&generation) && self.attempt.get(slot).is_some_and(Option::is_some)).then_some(slot)
    }

    fn attempting(&self, stream: Stream, subject: u64) -> bool {
        self.attempt.iter().flatten().any(|a| a.stream == stream && a.subject == subject)
    }

    /// Replaces one stream's wants with `wanted`, by subject: one walk of
    /// it beside the run held. Re-declaring the same set changes nothing, so
    /// producers say everything every turn. A kept subject keeps its enqueue
    /// time and retry wait; a repeated subject is one want, the later ask.
    pub fn declare(&mut self, stream: Stream, wanted: &[(u64, Ask)], at: f64) {
        xpute_kit::ensure!(wanted.windows(2).all(|p| p[0].0 <= p[1].0), ENOTRECOVERABLE, stream as u32);
        let (first, bound) = (self.first(stream), self.bounds[stream as usize]);
        let run = &self.desired[first..first + self.held[stream as usize]];
        let (mut k, mut n) = (0, 0);
        for (j, &(subject, ask)) in wanted.iter().enumerate() {
            if wanted.get(j + 1).is_some_and(|w| w.0 == subject) {
                continue;
            }
            while run.get(k).is_some_and(|w| w.subject < subject) {
                k += 1;
            }
            let w = match run.get(k) {
                Some(&w) if w.subject == subject => Wanted { ask, ..w },
                _ => Wanted {
                    subject,
                    ask,
                    enqueue_ms: at,
                    wait: Wait::Ready,
                },
            };
            xpute_kit::ensure!(n < bound, ENOTRECOVERABLE, stream as u32);
            self.merged[n] = w;
            n += 1;
        }
        self.desired[first..first + n].copy_from_slice(&self.merged[..n]);
        self.held[stream as usize] = n;
    }

    pub fn set_credits(&mut self, credits: u32) {
        self.credits = credits.min(MAX_CREDITS);
        self.most_credits = self.most_credits.max(self.credits);
    }

    /// `stream`'s requests waiting for a credit and out on one. For a
    /// report only, never for deciding what to want.
    pub fn stream_counts(&self, stream: Stream) -> (u32, u32) {
        let out = self.attempt.iter().flatten().filter(|a| a.stream == stream).count() as u32;
        let waiting = self.run(stream).iter().filter(|w| !self.attempting(stream, w.subject)).count() as u32;
        (waiting, out)
    }

    fn placed(&self, stream: Stream, w: &Wanted) -> Placed {
        Placed {
            rank: self.serve.rank(stream, w.subject, w.ask, w.enqueue_ms),
            stream,
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

        // A held server takes nothing; a failing one one probe at a time.
        if self.given_up || at < self.held_until {
            return;
        }
        let room = if self.failing.is_some() { 1 } else { self.credits as usize };
        let mut ready = core::mem::take(&mut self.ready);
        ready.clear();
        for s in Stream::ALL {
            let first = self.first(s);
            for (k, w) in self.run(s).iter().enumerate() {
                if w.wait.ready() && !self.attempting(s, w.subject) {
                    ready.push((self.placed(s, w), first + k));
                }
            }
        }
        ready.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let intake = self.serve.intake().max(1) as usize;
        for &(placed, i) in ready.iter().take(intake) {
            if self.attempt.iter().flatten().count() >= room {
                break;
            }
            let Some(slot) = self.free_slot() else { break };
            self.start(slot, placed.stream, self.desired[i], placed.rank, at);
        }
        self.ready = ready;
    }

    fn free_slot(&self) -> Option<usize> {
        (0..self.credits as usize).find(|&k| self.attempt[k].is_none())
    }

    fn start(&mut self, slot: usize, stream: Stream, w: Wanted, rank: Rank, at: f64) {
        self.attempt[slot] = Some(Attempt {
            stream,
            subject: w.subject,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        self.started.push((key_of(slot, self.generation[slot]), stream, w.subject));
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
        let want = self.find(a.stream, a.subject);
        match ended {
            Ended::Ok => {
                self.answered();
                if let Some(i) = want {
                    self.drop_want(a.stream, i);
                }
            }
            Ended::Failed(f) => self.failed(want, f, a.start_ms, at),
            // Still an answer from the server.
            Ended::Refused(refused) => {
                self.answered();
                if let Some(i) = want {
                    self.desired[i].wait = Wait::Room(refused);
                }
            }
            Ended::Aborted => {}
        }
        record(&a, at, ended, phases);
    }

    /// The server answered: whatever held it back is over.
    fn answered(&mut self) {
        self.failing = None;
        self.strikes = 0;
        self.struck_ms = f64::NEG_INFINITY;
        self.held_until = f64::NEG_INFINITY;
    }

    fn failed(&mut self, want: Option<usize>, failure: Failure, started_ms: f64, at: f64) {
        let seed = derive_seed_u64(self.strikes as u64, at.to_bits());
        let wait = match self.serve.blame(failure) {
            Blame::Object => Wait::Object,
            Blame::Network => {
                self.failing = Some(failure.errno);
                self.held_until = at + self.serve.backoff(1, seed).unwrap_or(0.0);
                return;
            }
            Blame::Server => {
                self.failing = Some(failure.errno);
                if started_ms <= self.struck_ms {
                    return;
                }
                self.struck_ms = at;
                self.strikes += 1;
                match self.serve.backoff(self.strikes, seed) {
                    Some(wait) => self.held_until = at + wait,
                    None => self.given_up = true,
                }
                return;
            }
        };
        // Still an answer from the server.
        self.answered();
        if let Some(i) = want {
            self.desired[i].wait = wait;
        }
    }

    /// The module is being reset under a host that will not answer what it was told to run.
    pub fn reset(&mut self) {
        for slot in 0..self.attempt.len() {
            if self.attempt[slot].is_some() {
                self.end(slot);
            }
        }
        self.held = [0; Stream::ALL.len()];
        self.answered();
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
        match ended {
            Ended::Failed(fail) => {
                row.fail = f64::from(fail.errno as i32);
                row.detail = fail.detail as f64;
            }
            Ended::Refused(_) => row.fail = f64::from(Errno::ENOMEM as i32),
            Ended::Ok | Ended::Aborted => {}
        }
        if let Some(at) = match ended {
            Ended::Failed(fail) => fail.at,
            Ended::Ok | Ended::Aborted | Ended::Refused(_) => None,
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

static FILES: Global<FnvHashMap<usize, Box<str>>> = Global::new();

/// A location's file name lies outside kernel memory on a device, so rows
/// name a heap copy; the names are few and kept for good.
fn file_in_memory(file: &'static str) -> &'static str {
    FILES.get(FnvHashMap::default).entry(file.as_ptr().addr()).or_insert_with(|| file.into())
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

/// The most credits the host has granted: a body on each may still be out
/// after a grant of fewer.
pub fn most_credits() -> u32 {
    sched().most_credits
}

pub fn stream_counts(stream: Stream) -> (u32, u32) {
    sched().stream_counts(stream)
}

pub fn pump() {
    let s = sched();
    s.pump(now());
    // Nothing else need come to end a hold.
    if s.held_until.is_finite() && !s.given_up {
        crate::sched::edge::wake_at(s.held_until);
    }
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

/// Whether the server failed past its last wait.
pub fn given_up() -> bool {
    sched().given_up
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
