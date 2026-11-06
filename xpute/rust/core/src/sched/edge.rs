// xpute-core/sched/edge.rs

//! A core's side of the turn: the deadline it keeps, tasks parked until the
//! next rising edge, and what the falling edge asks for.
//!
//! A parked task is woken at `rise`, never by itself: an executor that
//! re-reads its queue within one poll would run a self-woken task again past
//! the deadline. N is the core's task count; a task parks at most once.
//!
//! A task parked for the next turn asks for it at once (`next_turn`); one
//! that waits on its runtime asks for none (`runtime_turn`): the runtime begins the
//! next turn itself once it has done what the task waits for, as its read of
//! the completion ring kicks one, so a core on its own thread does not spin
//! while the runtime has not read.

use core::future::{poll_fn, Future};
use core::task::{Poll, Waker};

use super::wake::NO_WAKE;
use crate::clock::now;

pub struct Edge<const N: usize> {
    edge_at: f64,
    deadline_ms: f64,
    /// When the turn's tasks yield: the deadline's end, or later where the core
    /// kept them a share past what ran before them (`extend_to`).
    until: f64,
    parked: [Option<Waker>; N],
    parked_len: usize,
    runtime_parked: [Option<Waker>; N],
    runtime_parked_len: usize,
    resched: bool,
    wake_at: f64,
}

impl<const N: usize> Edge<N> {
    pub const fn new() -> Edge<N> {
        Edge {
            edge_at: 0.0,
            deadline_ms: 0.0,
            until: 0.0,
            parked: [const { None }; N],
            parked_len: 0,
            runtime_parked: [const { None }; N],
            runtime_parked_len: 0,
            resched: false,
            wake_at: f64::INFINITY,
        }
    }

    pub fn rise(&mut self, deadline_ms: f64) {
        self.edge_at = now();
        self.deadline_ms = deadline_ms;
        self.until = self.edge_at + deadline_ms;
        let runtime_parked = self.runtime_parked[..self.runtime_parked_len].iter_mut();
        for waker in self.parked[..self.parked_len]
            .iter_mut()
            .chain(runtime_parked)
        {
            if let Some(w) = waker.take() {
                w.wake();
            }
        }
        self.parked_len = 0;
        self.runtime_parked_len = 0;
    }

    /// Ms from now until the next turn is wanted: 0 when asked or parked for
    /// the next turn, else the earliest `wake_at`, else NO_WAKE. Reading
    /// clears it.
    pub fn fall(&mut self) -> f64 {
        let resched = core::mem::take(&mut self.resched) || self.parked_len > 0;
        let at = core::mem::replace(&mut self.wake_at, f64::INFINITY);
        match () {
            _ if resched => 0.0,
            _ if at.is_finite() => (at - now()).max(0.0),
            _ => NO_WAKE,
        }
    }

    pub fn edge_at(&self) -> f64 {
        self.edge_at
    }

    pub fn deadline_ms(&self) -> f64 {
        self.deadline_ms
    }

    pub fn is_spent(&self) -> bool {
        now() >= self.until
    }

    /// The tasks yield no sooner than `at`, whatever the deadline, so what
    /// overran it does not take the tasks' turn too.
    pub fn extend_to(&mut self, at: f64) {
        self.until = self.until.max(at);
    }

    pub fn ask_next(&mut self) {
        self.resched = true;
    }

    /// The earliest asked wins.
    pub fn wake_at(&mut self, at: f64) {
        self.wake_at = self.wake_at.min(at);
    }

    pub fn park(&mut self, waker: &Waker) {
        lay(&mut self.parked, &mut self.parked_len, waker);
    }

    /// Parked until a turn the runtime begins: `fall` asks for none for it.
    pub fn park_runtime(&mut self, waker: &Waker) {
        lay(
            &mut self.runtime_parked,
            &mut self.runtime_parked_len,
            waker,
        );
    }
}

fn lay<const N: usize>(wakers: &mut [Option<Waker>; N], len: &mut usize, waker: &Waker) {
    match wakers.get_mut(*len) {
        Some(slot) => {
            *slot = Some(waker.clone());
            *len += 1;
        }
        None => xpute_kit::bug!(ENOSPC, N),
    }
}

impl<const N: usize> Default for Edge<N> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn yield_if_spent<const N: usize>(
    edge: fn() -> &'static mut Edge<N>,
) -> impl Future<Output = ()> {
    park_if(edge, Edge::is_spent)
}

pub fn next_turn<const N: usize>(edge: fn() -> &'static mut Edge<N>) -> impl Future<Output = ()> {
    park_if(edge, |_| true)
}

/// Parked until the next turn the runtime begins, asking for none.
pub fn runtime_turn<const N: usize>(
    edge: fn() -> &'static mut Edge<N>,
) -> impl Future<Output = ()> {
    let mut parked = false;
    poll_fn(move |cx| {
        if parked {
            return Poll::Ready(());
        }
        parked = true;
        edge().park_runtime(cx.waker());
        Poll::Pending
    })
}

/// Parked until the next rise when `wait` says so. `edge` is a fn so no
/// borrow is held across the await.
fn park_if<const N: usize>(
    edge: fn() -> &'static mut Edge<N>,
    wait: fn(&Edge<N>) -> bool,
) -> impl Future<Output = ()> {
    let mut parked = false;
    poll_fn(move |cx| {
        if parked || !wait(edge()) {
            return Poll::Ready(());
        }
        parked = true;
        edge().park(cx.waker());
        Poll::Pending
    })
}

/// Waiting on it asks for no turn, where parking would ask for every frame.
/// One task a notify; a notice before the wait is kept.
pub struct Notify {
    waker: Option<Waker>,
    notified: bool,
}

impl Notify {
    pub const fn new() -> Notify {
        Notify {
            waker: None,
            notified: false,
        }
    }

    pub fn notify(&mut self) {
        self.notified = true;
        if let Some(w) = self.waker.take() {
            w.wake();
        }
    }
}

impl Default for Notify {
    fn default() -> Self {
        Self::new()
    }
}

pub fn notified(notify: fn() -> &'static mut Notify) -> impl Future<Output = ()> {
    poll_fn(move |cx| {
        let n = notify();
        if core::mem::take(&mut n.notified) {
            return Poll::Ready(());
        }
        n.waker = Some(cx.waker().clone());
        Poll::Pending
    })
}

#[cfg(test)]
#[path = "edge.test.rs"]
mod test;
