// pxl-core/sys/sched.rs

//! The core's own cooperative scheduling, on embassy's executor: the frame's
//! signal, a frame armed for a time, the entry's budget, and the yield that
//! leaves the entry.
//!
//! - `invalidate` asks for a frame: the frame task waits on it, then on the
//!   display's next vsync, and runs the world's frame (frame.rs).
//! - `invalidate_at` asks for a frame at a time. Only the earliest time is
//!   armed, by the timer task's one `monotonic-clock.wait-for` of xpute's, its
//!   delay rounded up; an earlier time asked meanwhile arms the task again,
//!   and the wait it lets go answers no one.
//! - Each entry has a budget, from its start (`begin`, link.rs `enter`): `spent`
//!   reads it, `run` spends it on a list once one item has done work, and
//!   `yield_if_spent` leaves the entry once it is spent.

pub mod display;
pub mod executor;
pub mod param;
#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
pub mod tick;

use core::future::{poll_fn, Future};
use core::pin::pin;
use core::task::{Poll, Waker};

use pxl_global::Global;
use xpute_guest::abi::clocks::monotonic_clock::wait_for;
use xpute_guest::clock::{duration, now};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::sched::param::BUDGET_MS;

struct Sched {
    /// Whether a frame is asked for, and the frame task's waker.
    invalid: bool,
    frame_waker: Option<Waker>,
    /// The earliest time a frame is armed for, or infinity, and the timer
    /// task's waker.
    earliest: f64,
    timer_waker: Option<Waker>,
    /// The entry's deadline.
    deadline: f64,
}

static SCHED: Global<Sched> = Global::new();

fn sched() -> &'static mut Sched {
    SCHED.get(|| Sched {
        invalid: false,
        frame_waker: None,
        earliest: f64::INFINITY,
        timer_waker: None,
        deadline: f64::INFINITY,
    })
}

/// Asks for a frame.
pub fn invalidate() {
    let s = sched();
    s.invalid = true;
    if let Some(w) = s.frame_waker.take() {
        w.wake();
    }
}

/// Whether a frame is asked for and not yet taken.
pub fn is_invalid() -> bool {
    sched().invalid
}

/// Waits until a frame is asked for, and takes the ask.
pub async fn invalidated() {
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

/// Asks for a frame at `at`, on the clock.
pub fn invalidate_at(at: f64) {
    let s = sched();
    if at >= s.earliest {
        return;
    }
    s.earliest = at;
    if let Some(w) = s.timer_waker.take() {
        w.wake();
    }
}

/// Starts the timer task.
pub fn boot() {
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
            invalidate();
        }
    }
}

/// Starts the entry's budget.
pub fn begin() {
    sched().deadline = now() + BUDGET_MS;
}

pub fn spent() -> bool {
    now() >= sched().deadline
}

/// What `run` came to: the items it stepped, and whether the budget stopped
/// it before the list's end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Run {
    pub step_cnt: u32,
    pub stopped_early: bool,
}

/// Steps `items` in order until the entry's budget is spent. `step` answers
/// whether it did work, and the budget holds only once one step has, so the
/// list always moves.
pub fn run<T>(items: impl IntoIterator<Item = T>, mut step: impl FnMut(T) -> bool) -> Run {
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
pub async fn yield_if_spent() {
    if spent() {
        // A wait that fails leaves the entry all the same.
        _ = wait_for(0).await;
    }
}
