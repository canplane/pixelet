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
//! - Each entry has an end, by one rule (`Budget`): the margin before the
//!   next frame's start where the core knows it, and one slice from the
//!   entry's own start where it does not (`begin`, link.rs `enter`; `set_pace_ms`,
//!   `begin_frame`); and once a frame has run its entry is spent
//!   (`end_frame`), so nothing stands between the frame and the entry's end,
//!   where its stream leaves for the GPU. `spent` reads the end, `run` spends
//!   the budget on a list once one item has done work, and `yield_if_spent`
//!   leaves the entry once it is spent.
//! - An entry that draws a frame does nothing else. A vsync's answer wakes
//!   the frame task alone, and every other task awaits `yield_if_spent`
//!   before its first work and between its units, so one the frame readied
//!   leaves the frame's spent entry with no work done and works in an entry
//!   that draws no frame, on that entry's budget.

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
use crate::sys::sched::param::{MARGIN_SHARE, SLICE_SHARE};

struct Sched {
    /// Whether a frame is asked for, and the frame task's waker.
    invalid: bool,
    frame_waker: Option<Waker>,
    /// Whether the frame task is in a frame: it took an ask (`invalidated`),
    /// and the frame has not run (`end_frame`).
    in_frame: bool,
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
        in_frame: false,
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
            s.in_frame = true;
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

/// The entry's end, by one rule: the margin, `MARGIN_SHARE` of the frame's
/// target interval, before the next frame's start where the core knows that
/// start, and one slice, `SLICE_SHARE` of what the margin leaves of the
/// interval, from the entry's own start where it does not.
///
/// The core knows the next frame's start only while a frame is due, and only
/// as one target interval after the last frame's start: the display ticks
/// there, the target being a whole number of its intervals from a tick, and
/// the frame task asks for the first vsync from then (rate.rs `due_at`). The
/// last frame is the one that started last, so a frame's own entry ends the
/// margin before one target from the frame's own start. A vsync comes within
/// half a display interval of that start (rate.rs `due_at`): an entry that
/// starts half a display interval past it or later, with no frame started
/// there, knows no start. So it is with a view that
/// stood still before the ask, a frame that ran past its target, the time
/// before any frame, and a page whose display does not tick while it is
/// hidden: the frame starts at a tick the core cannot place, and the entry
/// has its slice, as where no frame is due.
///
/// An entry that starts past its end ends at its start, and so does the entry
/// that drew a frame, from the frame's end. A spent entry still moves each
/// task by one part (`run`, `yield_if_spent`), so a part may end past the
/// next frame's start.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Budget {
    /// When the entry started; infinity before any.
    entry_at: f64,
    /// When the last frame started, run or refused; -infinity before any.
    frame_at: f64,
    /// The display's interval the last frame asked for was paced on, in ms;
    /// 0 before any.
    display_ms: f64,
    /// The target interval it was paced on, in ms; 0 before any.
    target_ms: f64,
    /// Whether the entry drew a frame.
    drawn: bool,
}

impl Budget {
    /// No entry has started, no frame, and none was asked for.
    const NONE: Budget = Budget {
        entry_at: f64::INFINITY,
        frame_at: f64::NEG_INFINITY,
        display_ms: 0.0,
        target_ms: 0.0,
        drawn: false,
    };

    fn begin(&mut self, entry_at: f64) {
        self.entry_at = entry_at;
        self.drawn = false;
    }

    /// When the entry ends, a frame being `due` or not.
    fn end(&self, due: bool) -> f64 {
        let margin_ms = MARGIN_SHARE * self.target_ms;
        let next_at = self.frame_at + self.target_ms;
        let end = if self.drawn {
            self.entry_at
        } else if due && self.entry_at < next_at + self.display_ms / 2.0 {
            next_at - margin_ms
        } else {
            self.entry_at + SLICE_SHARE * (self.target_ms - margin_ms)
        };
        end.max(self.entry_at)
    }
}

impl Sched {
    /// Whether a frame is due: one is asked for, or the frame task took the
    /// ask and its frame has not run.
    fn due(&self) -> bool {
        self.invalid || self.in_frame
    }

    /// When the entry ends.
    fn end(&self) -> f64 {
        self.budget.end(self.due())
    }
}

/// Starts the entry's budget.
pub(crate) fn begin() {
    sched().budget.begin(now());
}

/// The frame asked for is paced on the display's interval and a target
/// interval, as the frame task asks the display for its vsync.
pub(crate) fn set_pace_ms((display_ms, target_ms): (f64, f64)) {
    let b = &mut sched().budget;
    (b.display_ms, b.target_ms) = (display_ms, target_ms);
}

/// A frame starts at `at`, run or refused, and is the last frame from here.
pub(crate) fn begin_frame(at: f64) {
    sched().budget.frame_at = at;
}

/// The frame has run, and its entry is spent: a task the frame readied
/// leaves it before its first work (`yield_if_spent`), and the next entry has
/// its end by the rule, however far the frame ran past its own.
pub(crate) fn end_frame() {
    let s = sched();
    s.in_frame = false;
    s.budget.drawn = true;
}

/// When the last frame started, run or refused; -infinity before any.
pub(crate) fn frame_at() -> f64 {
    sched().budget.frame_at
}

/// The entry's budget, from its start to its end, in ms; 0 for an entry that
/// started past its end.
pub(crate) fn budget_ms() -> f64 {
    let s = sched();
    s.end() - s.budget.entry_at
}

pub(crate) fn spent() -> bool {
    now() >= sched().end()
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
