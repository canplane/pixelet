// pxl-core/sys/sched.rs

//! The core's own cooperative scheduling, on embassy's executor: the frame's
//! signal, a frame armed for a time, the entry's budget, and the yield that
//! leaves the entry.
//!
//! - `invalidate` asks for a frame: the frame task waits on it, then on the
//!   display's first vsync from when the frame is due, and runs the world's
//!   frame (frame.rs).
//! - `invalidate_at` asks for a frame at a time. Only the earliest time is
//!   armed, by the timer task's one `monotonic-clock.wait-for` of xpute's, its
//!   delay rounded up; an earlier time asked meanwhile arms the task again,
//!   and the wait it lets go answers no one.
//! - Each entry has a deadline (`Budget`): what the last frame's budget leaves
//!   of it, and at least the content's share of that budget from the entry's
//!   start (`begin`, link.rs `enter`). A frame takes the deadline again from
//!   its own start (`begin_frame`), the visible phase leaves the content's
//!   share unspent (`reserve`), and once the frame has run, what is left of
//!   the entry gets that share from then on (`end_frame`). `spent` reads the
//!   deadline, `run` spends it on a list once one item has done work, and
//!   `yield_if_spent` leaves the entry once it is spent.

pub(crate) mod display;
pub(crate) mod executor;
pub(crate) mod param;
pub(crate) mod tick;

use core::future::poll_fn;
use core::pin::pin;
use core::task::{Poll, Waker};

use pxl_global::Global;
use xpute_guest::abi::clocks::monotonic_clock::wait_for;
use xpute_guest::clock::{duration, now};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::spec::step::InvalidationReason;
use crate::sys::sched::param::{CONTENT_SHARE, MARGIN_SHARE};

struct Sched {
    /// Whether a frame is asked for, and the frame task's waker.
    invalid: bool,
    frame_waker: Option<Waker>,
    /// How many times a frame was asked for since the count was last taken,
    /// by its reason. The profiler alone reads it.
    invalidations: [u32; InvalidationReason::ALL.len()],
    /// The earliest time a frame is armed for, or infinity, why it is, and
    /// the timer task's waker.
    earliest: f64,
    earliest_reason: InvalidationReason,
    timer_waker: Option<Waker>,
    budget: Budget,
}

static SCHED: Global<Sched> = Global::new();

fn sched() -> &'static mut Sched {
    SCHED.get(|| Sched {
        invalid: false,
        frame_waker: None,
        invalidations: [0; InvalidationReason::ALL.len()],
        earliest: f64::INFINITY,
        earliest_reason: InvalidationReason::Pending,
        timer_waker: None,
        budget: Budget::NONE,
    })
}

/// Asks for a frame, saying why.
pub(crate) fn invalidate(reason: InvalidationReason) {
    let s = sched();
    s.invalid = true;
    s.invalidations[reason as usize] += 1;
    if let Some(w) = s.frame_waker.take() {
        w.wake();
    }
}

/// Whether a frame is asked for and not yet taken.
pub(crate) fn is_invalid() -> bool {
    sched().invalid
}

/// Waits until a frame is asked for, and takes the ask.
pub(crate) async fn invalidated() {
    poll_fn(|cx| {
        let s = sched();
        if core::mem::take(&mut s.invalid) {
            return Poll::Ready(());
        }
        s.frame_waker = Some(cx.waker().clone());
        Poll::Pending
    })
    .await;
}

/// How many times a frame was asked for since this was last called, by why.
pub(crate) fn take_invalidations() -> [u32; InvalidationReason::ALL.len()] {
    core::mem::take(&mut sched().invalidations)
}

/// Asks for a frame at `at`, on the clock, saying why.
pub(crate) fn invalidate_at(at: f64, reason: InvalidationReason) {
    let s = sched();
    if at >= s.earliest {
        return;
    }
    (s.earliest, s.earliest_reason) = (at, reason);
    if let Some(w) = s.timer_waker.take() {
        w.wake();
    }
}

/// Starts the timer task.
pub(crate) fn boot() {
    executor::spawner().spawn(timer().or_bug(Errno::ENOSPC));
}

/// Waits for the earliest time a frame is armed for, and asks for the frame
/// once it passes; an earlier time asked meanwhile arms it again.
#[embassy_executor::task]
async fn timer() {
    loop {
        let at = poll_fn(|cx| {
            let s = sched();
            if s.earliest.is_finite() {
                return Poll::Ready(s.earliest);
            }
            s.timer_waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await;
        let mut armed = pin!(wait_for(duration(at - now())));
        poll_fn(|cx| {
            if sched().earliest != at || armed.as_mut().poll(cx).is_ready() {
                return Poll::Ready(());
            }
            sched().timer_waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await;
        let s = sched();
        if s.earliest == at {
            s.earliest = f64::INFINITY;
            invalidate(s.earliest_reason);
        }
    }
}

/// The entry's deadline, by one rule: with a frame's budget of `frame_ms`,
/// the last frame's target interval less `MARGIN_SHARE`, it is what that
/// budget leaves from the last frame's start, and at least `CONTENT_SHARE` of
/// it from the entry's start, or from the frame's end once a frame has run.
/// While `reserved`, the visible phase's, it ends `CONTENT_SHARE` of the
/// budget sooner.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Budget {
    /// When the entry started.
    entry_at: f64,
    /// When the last frame started, run or refused; -infinity before any.
    frame_at: f64,
    /// The frame's budget, in ms.
    frame_ms: f64,
    deadline: f64,
    reserved: bool,
}

impl Budget {
    /// No entry has started and no frame.
    const NONE: Budget = Budget {
        entry_at: 0.0,
        frame_at: f64::NEG_INFINITY,
        frame_ms: 0.0,
        deadline: f64::INFINITY,
        reserved: false,
    };

    fn begin(&mut self, entry_at: f64, target_ms: f64) {
        self.entry_at = entry_at;
        self.frame_ms = target_ms * (1.0 - MARGIN_SHARE);
        self.deadline_from(entry_at);
    }

    fn begin_frame(&mut self, at: f64, target_ms: f64) {
        self.frame_at = at;
        self.frame_ms = target_ms * (1.0 - MARGIN_SHARE);
        self.deadline_from(self.entry_at);
    }

    /// Takes the deadline by the rule, the content's share counted from
    /// `from`.
    fn deadline_from(&mut self, from: f64) {
        self.deadline = (self.frame_at + self.frame_ms).max(from + CONTENT_SHARE * self.frame_ms);
    }

    /// When the budget is spent.
    fn end(&self) -> f64 {
        match self.reserved {
            true => self.deadline - CONTENT_SHARE * self.frame_ms,
            false => self.deadline,
        }
    }
}

/// Starts the entry's budget, on a frame target interval of `target_ms`.
pub(crate) fn begin(target_ms: f64) {
    sched().budget.begin(now(), target_ms);
}

/// A frame starts at `at`, on a target interval of `target_ms`, run or
/// refused: the entry's deadline is taken again from it.
pub(crate) fn begin_frame(at: f64, target_ms: f64) {
    sched().budget.begin_frame(at, target_ms);
}

/// The frame has run: what is left of the entry gets the content's share
/// from now, however far the frame ran past its deadline.
pub(crate) fn end_frame() {
    sched().budget.deadline_from(now());
}

/// When the last frame started, run or refused; -infinity before any.
pub(crate) fn frame_at() -> f64 {
    sched().budget.frame_at
}

/// Holds the content's share of the frame's budget back while `on`.
pub(crate) fn reserve(on: bool) {
    sched().budget.reserved = on;
}

/// The entry's deadline, in ms from its start.
pub(crate) fn deadline_ms() -> f64 {
    let b = sched().budget;
    b.deadline - b.entry_at
}

pub(crate) fn spent() -> bool {
    now() >= sched().budget.end()
}

/// What `run` came to: the items it stepped, and whether the budget stopped
/// it before the list's end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Run {
    pub step_cnt: u32,
    pub stopped_early: bool,
}

/// Steps `items` in order until the entry's budget is spent. `step` answers
/// whether it did work, and the budget holds only once one step has, so the
/// list always moves.
pub(crate) fn run<T>(items: impl IntoIterator<Item = T>, mut step: impl FnMut(T) -> bool) -> Run {
    let mut r = Run::default();
    let mut worked = false;
    for item in items {
        if worked && spent() {
            r.stopped_early = true;
            break;
        }
        worked |= step(item);
        r.step_cnt += 1;
    }
    r
}

/// Leaves the entry once its budget is spent, and comes back in a later one.
pub(crate) async fn yield_if_spent() {
    if spent() {
        // A wait that fails leaves the entry all the same.
        _ = wait_for(0).await;
    }
}

#[cfg(test)]
#[path = "sched.test.rs"]
mod test;
