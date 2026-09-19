// xpute-runtime/sched/edge.rs

//! A guest's side of the turn the host grants it (sched/quantum.rs): the
//! rising edge's quota as a deadline on the host clock, the tasks parked
//! until the next rising edge, and what the falling edge asks for.
//!
//! This is the interface a guest's work is written against, whichever
//! executor polls it, the way `GlobalAlloc` is whichever allocator serves it.
//! A task is a plain `Future`; it yields with `yield_if_spent` or
//! `next_turn`, and nothing here names an executor. The module that
//! assembles the guest picks one and polls it between `rise` and `fall`.
//!
//! **A parked task is woken on the next rising edge, not by itself.** A task
//! that wakes itself is polled again by whichever executor re-reads its queue
//! within one poll, past the quota; one woken at `rise` runs only in the turn
//! after. The table is fixed at N, the guest's number of tasks: a task awaits
//! one thing at a time and so is parked at most once, and a table that fills
//! is a guest that spawned more than it declared, which aborts.
//!
//! What the falling edge carries: 0 when a turn asked for the next frame, the
//! delay to the earliest `wake_at` otherwise, NO_WAKE for neither. Parked
//! tasks ask for the next frame by being parked.

use core::future::{poll_fn, Future};
use core::task::{Poll, Waker};

use super::quantum::NO_WAKE;
use crate::clock::now;

pub struct Edge<const N: usize> {
    edge_at: f64,
    quota_ms: f64,
    parked: [Option<Waker>; N],
    parked_len: usize,
    asked: bool,
    wake_at: f64,
}

impl<const N: usize> Edge<N> {
    pub const fn new() -> Edge<N> {
        Edge {
            edge_at: 0.0,
            quota_ms: 0.0,
            parked: [const { None }; N],
            parked_len: 0,
            asked: false,
            wake_at: f64::INFINITY,
        }
    }

    /// The rising edge: the turn's quota from now, and every task parked for
    /// it woken.
    pub fn rise(&mut self, quota_ms: f64) {
        self.edge_at = now();
        self.quota_ms = quota_ms;
        for waker in self.parked[..self.parked_len].iter_mut() {
            if let Some(w) = waker.take() {
                w.wake();
            }
        }
        self.parked_len = 0;
    }

    /// The falling edge: when the next turn is wanted, in ms from now. Reading
    /// clears it.
    pub fn fall(&mut self) -> f64 {
        let asked = core::mem::take(&mut self.asked) || self.parked_len > 0;
        let at = core::mem::replace(&mut self.wake_at, f64::INFINITY);
        match () {
            _ if asked => 0.0,
            _ if at.is_finite() => (at - now()).max(0.0),
            _ => NO_WAKE,
        }
    }

    /// When the turn rose.
    pub fn edge_at(&self) -> f64 {
        self.edge_at
    }

    pub fn quota_ms(&self) -> f64 {
        self.quota_ms
    }

    /// What is left of the quota, never below zero.
    pub fn remaining(&self) -> f64 {
        (self.edge_at + self.quota_ms - now()).max(0.0)
    }

    pub fn spent(&self) -> bool {
        now() >= self.edge_at + self.quota_ms
    }

    /// Asks for the next frame's turn.
    pub fn ask_next(&mut self) {
        self.asked = true;
    }

    /// Asks for a turn once the host clock reaches `at`: a wait only time ends.
    /// The earliest asked wins.
    pub fn wake_at(&mut self, at: f64) {
        self.wake_at = self.wake_at.min(at);
    }

    /// Wakes `waker` on the next rising edge.
    pub fn park(&mut self, waker: &Waker) {
        match self.parked.get_mut(self.parked_len) {
            Some(slot) => {
                *slot = Some(waker.clone());
                self.parked_len += 1;
            }
            None => std::process::abort(),
        }
    }
}

impl<const N: usize> Default for Edge<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Ready at once while the quota lasts; once it is spent, pending until the
/// next turn. `edge` is asked for the edge at each poll, so no borrow of it is
/// held across the await.
pub fn yield_if_spent<const N: usize>(edge: fn() -> &'static mut Edge<N>) -> impl Future<Output = ()> {
    let mut parked = false;
    poll_fn(move |cx| {
        if parked || !edge().spent() {
            return Poll::Ready(());
        }
        parked = true;
        edge().park(cx.waker());
        Poll::Pending
    })
}

/// Pending until the next turn, whatever is left of this one.
pub fn next_turn<const N: usize>(edge: fn() -> &'static mut Edge<N>) -> impl Future<Output = ()> {
    let mut parked = false;
    poll_fn(move |cx| {
        if parked {
            return Poll::Ready(());
        }
        parked = true;
        edge().park(cx.waker());
        Poll::Pending
    })
}

/// Where a task with nothing to do waits for something to arrive, one task a
/// notify. It asks for no turn while it waits: a task parked for the next
/// turn instead would ask for every frame, and an idle guest would never be
/// left alone. A notice that comes before the task waits is kept for it.
pub struct Notify {
    waker: Option<Waker>,
    notified: bool,
}

impl Notify {
    pub const fn new() -> Notify {
        Notify { waker: None, notified: false }
    }

    /// Something arrived: the waiting task is woken, or the next to wait
    /// returns at once.
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

/// Pending until `notify` is notified. `notify` is asked for at each poll, so
/// no borrow of it is held across the await.
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
