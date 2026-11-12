// pxl-core/sys/sched.rs

//! The core's own cooperative scheduling, on embassy's executor: the frame's
//! signal, a frame armed for a time, the entry's budget, and the yield that
//! leaves the entry.
//!
//! - `invalidate` asks for a frame: the frame task waits on it, then on the
//!   display's next vsync, and runs the world's frame (frame.rs).
//! - `invalidate_at` asks for a frame at a time. Only the earliest time is
//!   armed, by one `timer.sleep` of xpute's, its delay rounded up; a sleep
//!   armed for any other time does nothing when it wakes, and an earlier time
//!   arms one more.
//! - Each entry has a budget, from its start (`begin`, link.rs `enter`): `spent`
//!   reads it, `run` spends it on a list once one item has done work, and
//!   `yield_if_spent` leaves the entry once it is spent.

pub mod display;
pub mod executor;
pub mod param;
#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
pub mod tick;

use core::future::poll_fn;
use core::task::{Poll, Waker};
use std::cell::Cell;

use xpute_core::abi::timer::{sleep, sleep_then};
use xpute_core::clock::now;

use crate::sys::sched::param::BUDGET_MS;

thread_local! {
    /// Whether a frame is asked for, and the frame task's waker.
    static INVALID: Cell<bool> = const { Cell::new(false) };
    static FRAME_WAKER: Cell<Option<Waker>> = const { Cell::new(None) };
    /// The earliest time a frame is armed for, or infinity.
    static EARLIEST: Cell<f64> = const { Cell::new(f64::INFINITY) };
    /// The entry's deadline.
    static DEADLINE: Cell<f64> = const { Cell::new(f64::INFINITY) };
}

/// Asks for a frame.
pub fn invalidate() {
    INVALID.set(true);
    if let Some(w) = FRAME_WAKER.take() {
        w.wake();
    }
}

/// Whether a frame is asked for and not yet taken.
pub fn is_invalid() -> bool {
    INVALID.get()
}

/// Waits until a frame is asked for, and takes the ask.
pub async fn invalidated() {
    poll_fn(|cx| {
        if INVALID.replace(false) {
            return Poll::Ready(());
        }
        FRAME_WAKER.set(Some(cx.waker().clone()));
        Poll::Pending
    })
    .await;
}

/// Asks for a frame at `at`, on the clock.
pub fn invalidate_at(at: f64) {
    if at >= EARLIEST.get() {
        return;
    }
    EARLIEST.set(at);
    let ms = (at - now()).max(0.0).ceil() as u32;
    sleep_then(ms, move |_| {
        if EARLIEST.get() == at {
            EARLIEST.set(f64::INFINITY);
            invalidate();
        }
    });
}

/// Starts the entry's budget.
pub fn begin() {
    DEADLINE.set(now() + BUDGET_MS);
}

pub fn spent() -> bool {
    now() >= DEADLINE.get()
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
        // A timer that fails leaves the entry all the same.
        _ = sleep(0).await;
    }
}
