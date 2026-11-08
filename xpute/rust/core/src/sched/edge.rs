// xpute-core/sched/edge.rs

//! This is the core's side of a turn. A turn begins at its rising edge
//! (`rise`), which sets its deadline, and ends at its falling edge (`fall`),
//! which answers when the core wants the next turn. A task that must wait for
//! a later turn parks here in between.
//!
//! Only `rise` wakes a parked task; a task never wakes itself. An executor
//! that re-reads its queue within one poll would otherwise run a self-woken
//! task again, past the deadline. A task parks at most once a turn, so the
//! edge never holds more wakers than the core has tasks; its lists grow to
//! the most ever parked at once and keep that room.
//!
//! A task parked by `next_turn` asks for the next turn at once. A task parked
//! by `runtime_turn` waits on the runtime and asks for no turn: the runtime
//! starts the next turn itself once it has done what the task waits for,
//! because its read of the completion ring kicks the core. A core on its own
//! thread therefore does not spin while the runtime has not yet read.

use core::future::{poll_fn, Future};
use core::task::{Poll, Waker};

use super::wake::NO_WAKE;
use crate::clock::now;

pub struct Edge {
    edge_at: f64,
    deadline_ms: f64,
    /// The time at which the turn's tasks yield: the deadline's end, or later
    /// where `extend_to` kept the tasks their share after earlier work overran.
    until: f64,
    parked: Vec<Waker>,
    runtime_parked: Vec<Waker>,
    resched: bool,
    wake_at: f64,
}

impl Edge {
    pub const fn new() -> Edge {
        Edge {
            edge_at: 0.0,
            deadline_ms: 0.0,
            until: 0.0,
            parked: Vec::new(),
            runtime_parked: Vec::new(),
            resched: false,
            wake_at: f64::INFINITY,
        }
    }

    pub fn rise(&mut self, deadline_ms: f64) {
        self.edge_at = now();
        self.deadline_ms = deadline_ms;
        self.until = self.edge_at + deadline_ms;
        for w in self.parked.drain(..).chain(self.runtime_parked.drain(..)) {
            w.wake();
        }
    }

    /// Answers in how many ms the core wants the next turn: 0 when a task
    /// asked or parked for it, else the time until the earliest `wake_at`,
    /// else NO_WAKE. It clears what it answers, so every turn asks afresh.
    pub fn fall(&mut self) -> f64 {
        let resched = core::mem::take(&mut self.resched) || !self.parked.is_empty();
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

    /// Keeps the tasks running until at least `at`, past the deadline if need
    /// be, so that work which overran the deadline does not also take the
    /// tasks' share of the turn.
    pub fn extend_to(&mut self, at: f64) {
        self.until = self.until.max(at);
    }

    pub fn ask_next(&mut self) {
        self.resched = true;
    }

    pub fn wake_at(&mut self, at: f64) {
        self.wake_at = self.wake_at.min(at);
    }

    pub fn park(&mut self, waker: &Waker) {
        self.parked.push(waker.clone());
    }

    /// Parks a task until the runtime begins a turn. `fall` asks for no turn
    /// on its behalf.
    pub fn park_runtime(&mut self, waker: &Waker) {
        self.runtime_parked.push(waker.clone());
    }
}

impl Default for Edge {
    fn default() -> Self {
        Self::new()
    }
}

pub fn yield_if_spent(edge: fn() -> &'static mut Edge) -> impl Future<Output = ()> {
    park_if(edge, Edge::is_spent)
}

pub fn next_turn(edge: fn() -> &'static mut Edge) -> impl Future<Output = ()> {
    park_if(edge, |_| true)
}

/// Waits for the next turn the runtime begins, without asking for one.
pub fn runtime_turn(edge: fn() -> &'static mut Edge) -> impl Future<Output = ()> {
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

/// Parks until the next rise when `wait` says so. `edge` is passed as a
/// function so that no borrow of the edge is held across the await.
fn park_if(edge: fn() -> &'static mut Edge, wait: fn(&Edge) -> bool) -> impl Future<Output = ()> {
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

/// A wait for a notice that asks for no turn meanwhile, so an idle core is
/// left alone; parking instead would ask for every turn. One task at a time
/// waits on a `Notify`, and a notice that comes before the wait is kept for it.
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
