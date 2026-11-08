// pxl-core/sys/io/request.rs

//! What each stream wants, replaced whole each turn, and what is out on a
//! credit. `pump` stops what nobody wants and starts the rest in the policy's
//! order; what starts, stops and ends waits in `events` for the turn.
//!
//! An answer ends its want; a failure ends only its attempt. An object's
//! failure holds its want while it is wanted; a body refused room holds its
//! want until the room is given back; the network's or the server's holds
//! every start, then lets one probe out until something answers, and the
//! server's past its last wait gives the reads up. What is out is never
//! preempted.

use std::collections::VecDeque;

use crate::sys::io::demand::{Order, Phases, Source, Stream};
use crate::sys::io::failure::{Ended, Failure};
use crate::sys::io::param::{BODY_SLOTS, MAX_CREDITS};
use crate::sys::io::policy::{self, Fault, Rank};
use crate::sys::io::room::Refusal;
use crate::sys::load::LOAD_FILES;
use crate::sys::sched::edge;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_kit::math::rng::derive_seed_u64;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const STREAMS: usize = Stream::ALL.len();
const SLOTS: usize = MAX_CREDITS as usize;

#[derive(Clone, Copy)]
struct Want {
    id: u64,
    order: Order,
    /// First wanted, kept across declarations: the wait the profiler reports.
    enqueue_ms: f64,
    wait: Wait,
}

#[derive(Clone, Copy)]
enum Wait {
    Ready,
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

/// A key is a slot and its generation, valid while its attempt is out.
#[derive(Clone, Copy)]
struct Slot {
    gen: u32,
    out: Option<Attempt>,
}

pub enum Event {
    /// Its key, for its reader to open.
    Start(u64, Stream, u64),
    /// Its key, for its reader to close.
    Stop(u64),
    /// When, as what, and the timings its run reported.
    End(Attempt, f64, Ended, Option<Phases>),
}

/// What holds every start back, gone with the next answer.
#[derive(Clone, Copy)]
struct Outage {
    failing: Option<Errno>,
    /// The server's failures, one a wait.
    failures: u32,
    struck_ms: f64,
    until: f64,
}

const NO_OUTAGE: Outage = Outage {
    failing: None,
    failures: 0,
    struck_ms: f64::NEG_INFINITY,
    until: f64::NEG_INFINITY,
};

struct Sched {
    /// By id, as many as `bounds` holds: a layer's its tile cache's slots, as
    /// it asks only for what it can hold; an asset's a file a slot.
    wants: [Vec<Want>; STREAMS],
    bounds: [usize; STREAMS],
    slots: [Slot; SLOTS],
    credits: u32,
    most_credits: u32,
    events: VecDeque<Event>,
    /// A declaration's and a pump's scratch, so a turn allocates nothing.
    fresh: Vec<Want>,
    ready: Vec<(Rank, Stream, Want)>,
    outage: Outage,
    /// The server failed past its last wait; only `reset` ends it.
    given_up: bool,
}

static SCHED: Global<Sched> = Global::new();

fn sched() -> &'static mut Sched {
    // Asked before `open` is the roof wired wrong, never a state to wait out.
    SCHED.get(|| None.or_bug(Errno::ENOTRECOVERABLE))
}

/// Opened once the core is laid out, with each layer's frames.
pub fn open(frames: impl Fn(Stream) -> u32) {
    let bounds = Stream::ALL.map(|s| match s.row().source {
        Source::Load => LOAD_FILES,
        Source::Bake(_) | Source::Objects => frames(s) as usize,
    });
    SCHED.set(Sched {
        wants: bounds.map(Vec::with_capacity),
        bounds,
        slots: [Slot { gen: 0, out: None }; SLOTS],
        credits: 0,
        most_credits: 0,
        events: VecDeque::new(),
        fresh: Vec::with_capacity(bounds.iter().copied().max().unwrap_or(0)),
        ready: Vec::with_capacity(bounds.iter().sum()),
        outage: NO_OUTAGE,
        given_up: false,
    });
}

/// `stream`'s wants are `wanted`, by id, a repeated id its last order. An id
/// kept keeps its time and its wait, so a producer says everything every turn.
pub fn declare(stream: Stream, wanted: &[(u64, Order)]) {
    let (s, at) = (sched(), now());
    let i = stream as usize;
    xpute_kit::ensure!(
        wanted.is_sorted_by_key(|w| w.0),
        ENOTRECOVERABLE,
        stream as u32
    );
    s.fresh.clear();
    let mut k = 0;
    for same in wanted.chunk_by(|a, b| a.0 == b.0) {
        let (id, order) = same[same.len() - 1];
        k += s.wants[i][k..].partition_point(|w| w.id < id);
        s.fresh.push(match s.wants[i].get(k) {
            Some(&w) if w.id == id => Want { order, ..w },
            _ => Want {
                id,
                order,
                enqueue_ms: at,
                wait: Wait::Ready,
            },
        });
    }
    xpute_kit::ensure!(s.fresh.len() <= s.bounds[i], ENOTRECOVERABLE, stream as u32);
    s.wants[i].clear();
    s.wants[i].extend_from_slice(&s.fresh);
}

pub fn set_credits(credits: u32) {
    let s = sched();
    s.credits = credits.min(MAX_CREDITS);
    s.most_credits = s.most_credits.max(s.credits);
}

/// One pump a turn, after every producer has spoken, or the first would fill
/// every slot.
pub fn pump() {
    let (s, at) = (sched(), now());
    // Abandoned work stops in the turn it is abandoned: after a zoom out and
    // back in, credits must not sit on the zoom passed through.
    for k in 0..SLOTS {
        if s.slots[k]
            .out
            .is_some_and(|a| find(s, a.stream, a.id).is_none())
        {
            stop(s, k, at);
        }
    }
    if s.given_up {
        return;
    }
    if at >= s.outage.until {
        start_ready(s, at);
    }
    // Nothing else need come to end a hold.
    if s.outage.until.is_finite() {
        edge::wake_at(s.outage.until);
    }
}

/// The best of what is ready on the free credits; a failing server gets one
/// probe at a time.
fn start_ready(s: &mut Sched, at: f64) {
    let credits = s.credits as usize;
    let out = s.slots.iter().filter(|c| c.out.is_some()).count();
    let free = s.slots[..credits]
        .iter()
        .filter(|c| c.out.is_none())
        .count();
    let room = if s.outage.failing.is_some() {
        1
    } else {
        credits
    };
    let n = free.min(room.saturating_sub(out));
    if n == 0 {
        return;
    }
    let mut ready = core::mem::take(&mut s.ready);
    ready.clear();
    for stream in Stream::ALL {
        for &w in &s.wants[stream as usize] {
            if w.wait.ready() && !attempting(s, stream, w.id) {
                ready.push((policy::rank(stream, w.id, w.order), stream, w));
            }
        }
    }
    // The stream and the id only so two runs of a turn start the same work.
    ready.sort_unstable_by(|a, b| {
        let tie = |r: &(Rank, Stream, Want)| (r.1 as u8, r.2.id);
        a.0.cmp(&b.0).then(tie(a).cmp(&tie(b)))
    });
    for &(rank, stream, w) in ready.iter().take(n) {
        let k = s.slots[..credits]
            .iter()
            .position(|c| c.out.is_none())
            .or_bug(Errno::ENOTRECOVERABLE);
        s.slots[k].out = Some(Attempt {
            stream,
            id: w.id,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        s.events
            .push_back(Event::Start(key(k, s.slots[k].gen), stream, w.id));
    }
    s.ready = ready;
}

/// One at a time, the scheduler let go between: a stop's reader settles here.
pub fn take_event() -> Option<Event> {
    sched().events.pop_front()
}

pub fn attempt_of(key: u64) -> Option<(Stream, u64)> {
    let s = sched();
    let a = s.slots[slot_of(s, key)?].out?;
    Some((a.stream, a.id))
}

/// `key`'s attempt is over: an answer ends its want, a failure holds it.
pub fn settle(key: u64, ended: Ended, phases: Option<Phases>) {
    let (s, at) = (sched(), now());
    let Some(k) = slot_of(s, key) else { return };
    let a = end(s, k);
    s.events.push_back(Event::End(a, at, ended, phases));
    let wait = match ended {
        Ended::Aborted => return,
        Ended::Ok => None,
        Ended::Refused(refused) => Some(Wait::Room(refused)),
        Ended::Failed(f) => match policy::fault(f) {
            Fault::Object => Some(Wait::Object),
            fault => return hold(s, fault, f, a.start_ms, at),
        },
    };
    // Each of these is the server's answer.
    s.outage = NO_OUTAGE;
    let Some(i) = find(s, a.stream, a.id) else {
        return;
    };
    let run = &mut s.wants[a.stream as usize];
    match wait {
        Some(wait) => run[i].wait = wait,
        None => {
            run.remove(i);
        }
    }
}

/// Every start held back. What was already out when the server's failure was
/// counted fails with it, so an outage is counted once.
fn hold(s: &mut Sched, fault: Fault, f: Failure, started_ms: f64, at: f64) {
    let o = &mut s.outage;
    let seed = derive_seed_u64(o.failures as u64, at.to_bits());
    o.failing = Some(f.errnum);
    if fault == Fault::Network {
        o.until = at + policy::backoff(1, seed).unwrap_or(0.0);
    } else if started_ms > o.struck_ms {
        o.struck_ms = at;
        o.failures += 1;
        match policy::backoff(o.failures, seed) {
            Some(wait) => o.until = at + wait,
            None => s.given_up = true,
        }
    }
}

/// The module is reset under a runtime that will not answer what it was told
/// to run: its starts and stops go with it, its ends stay the profiler's.
pub fn reset() {
    let s = sched();
    for k in 0..SLOTS {
        if s.slots[k].out.is_some() {
            end(s, k);
        }
    }
    s.wants.iter_mut().for_each(Vec::clear);
    s.outage = NO_OUTAGE;
    s.given_up = false;
    s.events.retain(|e| matches!(e, Event::End(..)));
}

/// Whether `stream`'s want of `id` waits on its object, which failed.
pub fn has_object_failed(stream: Stream, id: u64) -> bool {
    let s = sched();
    find(s, stream, id).is_some_and(|i| matches!(s.wants[stream as usize][i].wait, Wait::Object))
}

/// The body slots a tile asked again may come in: less one for each of the
/// most credits ever granted, as a body may still be out on each, and one for
/// each Earth tile the bound holds, whose packet waits for a frame
/// (sys/io/body.rs).
pub fn spare_slots() -> u32 {
    let s = sched();
    let earth = s.bounds[Stream::earth as usize] as u32;
    BODY_SLOTS.saturating_sub(s.most_credits.max(1) + earth)
}

/// Why reads fail, where none has come to an answer since one did.
pub fn failing() -> Option<Errno> {
    sched().outage.failing
}

pub fn given_up() -> bool {
    sched().given_up
}

const fn key(slot: usize, gen: u32) -> u64 {
    (slot as u64) << 32 | gen as u64
}

fn slot_of(s: &Sched, key: u64) -> Option<usize> {
    let k = (key >> 32) as usize;
    let slot = s.slots.get(k)?;
    (slot.gen == key as u32 && slot.out.is_some()).then_some(k)
}

fn find(s: &Sched, stream: Stream, id: u64) -> Option<usize> {
    s.wants[stream as usize]
        .binary_search_by_key(&id, |w| w.id)
        .ok()
}

fn attempting(s: &Sched, stream: Stream, id: u64) -> bool {
    s.slots
        .iter()
        .flat_map(|c| c.out)
        .any(|a| a.stream == stream && a.id == id)
}

/// Its key is spent: what the runtime still reports under it, an abort or a
/// body that raced the stop, is dropped.
fn end(s: &mut Sched, k: usize) -> Attempt {
    let slot = &mut s.slots[k];
    slot.gen = slot.gen.wrapping_add(1);
    slot.out.take().or_bug(Errno::ENOTRECOVERABLE)
}

fn stop(s: &mut Sched, k: usize, at: f64) {
    s.events.push_back(Event::Stop(key(k, s.slots[k].gen)));
    let a = end(s, k);
    s.events.push_back(Event::End(a, at, Ended::Aborted, None));
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
