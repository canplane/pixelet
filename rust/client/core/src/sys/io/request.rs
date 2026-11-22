// pxl-core/sys/io/request.rs

//! The request scheduler, the one authority over which reads are out. Each
//! stream declares its wants whole every frame, and each read holds a
//! credit from its start until it has ended at the host. `pump` stops what nobody wants any more and starts the
//! rest in the policy's order. What starts, stops and ends waits in `events`
//! until the frame takes it.
//!
//! An answer ends its want, and a failure ends only its attempt. After an
//! object fails, its want is held back for as long as it is wanted. A body
//! refused room holds its want back until room is given back. A failure of
//! the network or the server holds every start back, and then lets one probe
//! out at a time until something answers. Past the server's last wait, the
//! reads are given up. A read that is out is never stopped for one ranked
//! higher.

use crate::spec::step::InvalidationReason;
use std::collections::VecDeque;

use crate::sys::io::demand::{Order, PhaseList, Source, Stream};
use crate::sys::io::failure::Exit;
use crate::sys::io::param::{BODY_SLOT_CNT, CREDIT_CNT};
use crate::sys::io::policy::{self, Fault, Rank};
use crate::sys::io::room::Refusal;
use crate::sys::load::LOAD_FILE_CAP;
use crate::sys::sched;
use pxl_global::Global;
use xpute_guest::clock::now;
use xpute_kit::math::rng::seed_mix64;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const STREAM_CNT: usize = Stream::ALL.len();
const SLOT_CNT: usize = CREDIT_CNT as usize;

#[derive(Clone, Copy)]
struct Want {
    id: u64,
    order: Order,
    /// When the want was first declared. It is kept across declarations, and
    /// the profiler reports the wait from it.
    enqueue_ms: f64,
    wait: Wait,
}

#[derive(Clone, Copy)]
enum Wait {
    Ready,
    Room(Refusal),
    /// The object failed, and the want waits for as long as it is wanted.
    Object,
}

impl Wait {
    fn ready(self) -> bool {
        match self {
            Wait::Ready => true,
            Wait::Room(refused) => refused.released(),
            Wait::Object => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Attempt {
    pub stream: Stream,
    pub id: u64,
    pub rank: Rank,
    pub enqueue_ms: f64,
    pub start_ms: f64,
}

/// A credit's slot: one read at the host, from its start to its end there.
/// A key names a slot and its generation, and it is valid only while the
/// slot's attempt is out. `reading` is the key the slot's read started
/// under, until its reader says the read has ended (`end_read`): a read
/// stopped holds its credit until the host lets it go, so no more reads are
/// at the host than there are credits.
#[derive(Clone, Copy)]
struct Slot {
    generation: u32,
    out: Option<Attempt>,
    reading: Option<u64>,
}

impl Slot {
    fn is_free(&self) -> bool {
        self.out.is_none() && self.reading.is_none()
    }
}

pub(crate) enum Event {
    /// An attempt started under its key, for its reader to open.
    Start(u64, Stream, u64),
    /// An attempt stopped under its key, for its reader to close.
    Stop(u64),
    /// An attempt ended: when, how, and the timings its run reported.
    End(Attempt, f64, Exit, Option<PhaseList>),
}

/// The hold on every start after the network or the server failed. The next
/// answer lifts it.
#[derive(Clone, Copy)]
struct Outage {
    failing: Option<Errno>,
    /// The server's failures in a row, each of which began a wait.
    fail_cnt: u32,
    struck_ms: f64,
    until: f64,
}

const NO_OUTAGE: Outage = Outage {
    failing: None,
    fail_cnt: 0,
    struck_ms: f64::NEG_INFINITY,
    until: f64::NEG_INFINITY,
};

struct Sched {
    /// Each stream's wants, sorted by id, at most its bound in `bounds`. A
    /// layer's bound is its tile cache's slots, because a layer asks only for
    /// what it can hold. The assets' bound is the files that may be in flight
    /// at once (`LOAD_FILE_CAP`).
    wants: [Vec<Want>; STREAM_CNT],
    bounds: [usize; STREAM_CNT],
    slots: [Slot; SLOT_CNT],
    events: VecDeque<Event>,
    /// Scratch for `declare` and `pump`, so that a frame allocates nothing.
    fresh: Vec<Want>,
    ready: Vec<(Rank, Stream, Want)>,
    outage: Outage,
    /// The server failed after its last wait. Nothing clears this.
    given_up: bool,
}

static SCHED: Global<Sched> = Global::new();

fn sched() -> &'static mut Sched {
    // A call before `open` means the core's boot is wired wrong. It is never
    // a state to wait out.
    SCHED.get(|| None.or_bug(Errno::ENOTRECOVERABLE))
}

/// Opens the scheduler once the core's memory is laid out. `frames` gives
/// each layer's bound, the slots of its tile cache.
pub(crate) fn open(frames: impl Fn(Stream) -> u32) {
    let bounds = Stream::ALL.map(|s| match s.row().source {
        Source::Load => LOAD_FILE_CAP,
        Source::Base(_) | Source::Object => frames(s) as usize,
    });
    SCHED.set(Sched {
        wants: bounds.map(Vec::with_capacity),
        bounds,
        slots: [Slot {
            generation: 0,
            out: None,
            reading: None,
        }; SLOT_CNT],
        events: VecDeque::new(),
        fresh: Vec::with_capacity(bounds.iter().copied().max().unwrap_or(0)),
        ready: Vec::with_capacity(bounds.iter().sum()),
        outage: NO_OUTAGE,
        given_up: false,
    });
}

/// Replaces `stream`'s wants with `wanted`, which is sorted by id. Of a
/// repeated id, the last order counts. An id that stays keeps the time it was
/// first wanted and its wait, so a producer declares everything it wants
/// every frame.
pub(crate) fn declare(stream: Stream, wanted: &[(u64, Order)]) {
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

/// Runs once a frame, after every producer has declared. A pump after only
/// the first producer would fill every slot with that producer's wants.
pub(crate) fn pump() {
    let (s, at) = (sched(), now());
    // Abandoned work stops in the frame it is abandoned. After a zoom out and
    // back in, the credits must not stay on the zoom passed through.
    for k in 0..SLOT_CNT {
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
}

/// When a frame's pump is waited for at `at_ms`, if one is: the end of the
/// hold on every start. Nothing else may come to wake the core then. It is
/// asked every entry, since a timer lasts only until the next entry.
pub(crate) fn next_pump_at(at_ms: f64) -> Option<f64> {
    let until = sched().outage.until;
    (until > at_ms).then_some(until)
}

/// Starts the best ranked of the ready wants on the free credits. A failing
/// server gets one probe at a time.
fn start_ready(s: &mut Sched, at: f64) {
    let out = s.slots.iter().filter(|c| c.out.is_some()).count();
    let free = s.slots.iter().filter(|c| c.is_free()).count();
    let room = if s.outage.failing.is_some() {
        1
    } else {
        SLOT_CNT
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
    // The stream and the id break ties only so that two runs of the same frame
    // start the same work.
    ready.sort_unstable_by(|a, b| {
        let tie = |r: &(Rank, Stream, Want)| (r.1 as u8, r.2.id);
        a.0.cmp(&b.0).then(tie(a).cmp(&tie(b)))
    });
    for &(rank, stream, w) in ready.iter().take(n) {
        let k = s
            .slots
            .iter()
            .position(Slot::is_free)
            .or_bug(Errno::ENOTRECOVERABLE);
        s.slots[k].reading = Some(key(k, s.slots[k].generation));
        s.slots[k].out = Some(Attempt {
            stream,
            id: w.id,
            rank,
            enqueue_ms: w.enqueue_ms,
            start_ms: at,
        });
        s.events
            .push_back(Event::Start(key(k, s.slots[k].generation), stream, w.id));
    }
    s.ready = ready;
}

/// The next event. Events are taken one at a time, and the scheduler is not
/// borrowed between them, so the reader handling one may call back into it.
pub(crate) fn take_event() -> Option<Event> {
    sched().events.pop_front()
}

/// The read started under `key` has ended at the host, however it ended:
/// its reader says so once, and where no read was made for the attempt.
/// The slot's credit is free once its attempt has ended too, and a frame's
/// pump then starts what waited for it.
pub(crate) fn end_read(key: u64) {
    let s = sched();
    let Some(slot) = s.slots.get_mut((key >> 32) as usize) else {
        return;
    };
    if slot.reading == Some(key) {
        slot.reading = None;
        if slot.out.is_none() {
            sched::invalidate(InvalidationReason::Completion);
        }
    }
}

pub(crate) fn attempt_of(key: u64) -> Option<(Stream, u64)> {
    let s = sched();
    let a = s.slots[slot_of(s, key)?].out?;
    Some((a.stream, a.id))
}

/// Ends `key`'s attempt. An answer ends its want, and a failure holds the
/// want back.
pub(crate) fn settle(key: u64, exit: Exit, phases: Option<PhaseList>) {
    let (s, at) = (sched(), now());
    let Some(k) = slot_of(s, key) else { return };
    let a = end(s, k);
    s.events.push_back(Event::End(a, at, exit.clone(), phases));
    let wait = match exit {
        Exit::Aborted => return,
        Exit::Ok => None,
        Exit::Refused(refused) => Some(Wait::Room(refused)),
        Exit::Failed(f) => match policy::fault(&f) {
            Fault::Object => Some(Wait::Object),
            fault => return hold(s, fault, f.errnum, a.start_ms, at),
        },
    };
    // Each of these exits means the server answered, which lifts the outage.
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

/// Holds every start back after a failure of the network or the server. An
/// attempt that was already out when a server failure was counted fails with
/// it, so one outage is counted once.
fn hold(s: &mut Sched, fault: Fault, errnum: Errno, started_ms: f64, at: f64) {
    let o = &mut s.outage;
    let seed = seed_mix64(o.fail_cnt as u64, at.to_bits());
    o.failing = Some(errnum);
    if fault == Fault::Network {
        o.until = at + policy::backoff(1, seed).unwrap_or(0.0);
    } else if started_ms > o.struck_ms {
        o.struck_ms = at;
        o.fail_cnt += 1;
        match policy::backoff(o.fail_cnt, seed) {
            Some(wait) => o.until = at + wait,
            None => s.given_up = true,
        }
    }
    // A frame's pump shows the failure and starts a probe at once where the
    // hold has already ended or this failure began none.
    sched::invalidate(InvalidationReason::Completion);
}

/// Whether `stream`'s want of `id` is held back because its object failed.
pub(crate) fn has_object_failed(stream: Stream, id: u64) -> bool {
    let s = sched();
    find(s, stream, id).is_some_and(|i| matches!(s.wants[stream as usize][i].wait, Wait::Object))
}

/// The body slots that a tile asked for again may arrive in. The count
/// leaves out one slot for each credit, because a body may still be out on
/// each, and one for each imagery tile the imagery's bound holds, whose packet
/// waits for a frame.
pub(crate) fn spare_slot_cnt() -> u32 {
    let imagery = sched().bounds[Stream::Imagery as usize] as u32;
    BODY_SLOT_CNT.saturating_sub(CREDIT_CNT + imagery)
}

/// Why the reads are failing, or None once an answer has come since the last
/// failure.
pub(crate) fn failing() -> Option<Errno> {
    sched().outage.failing
}

pub(crate) fn given_up() -> bool {
    sched().given_up
}

const fn key(slot: usize, generation: u32) -> u64 {
    (slot as u64) << 32 | generation as u64
}

fn slot_of(s: &Sched, key: u64) -> Option<usize> {
    let k = (key >> 32) as usize;
    let slot = s.slots.get(k)?;
    (slot.generation == key as u32 && slot.out.is_some()).then_some(k)
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

/// Ends the attempt in slot `k` and spends its key. Whatever the host
/// still reports under the key, an abort or a body that raced the stop, is
/// dropped.
fn end(s: &mut Sched, k: usize) -> Attempt {
    let slot = &mut s.slots[k];
    slot.generation = slot.generation.wrapping_add(1);
    slot.out.take().or_bug(Errno::ENOTRECOVERABLE)
}

fn stop(s: &mut Sched, k: usize, at: f64) {
    s.events
        .push_back(Event::Stop(key(k, s.slots[k].generation)));
    let a = end(s, k);
    s.events.push_back(Event::End(a, at, Exit::Aborted, None));
}

#[cfg(test)]
#[path = "request.test.rs"]
mod test;
