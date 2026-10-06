// xpute-guest/sched/edge.rs

//! A guest's side of the turn: the quota as a deadline, tasks parked until the
//! next rising edge, and what the falling edge asks for.
//!
//! A parked task is woken at `rise`, never by itself: an executor that
//! re-reads its queue within one poll would run a self-woken task again past
//! the quota. N is the guest's task count; a task parks at most once.

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

    /// Ms from now until the next turn is wanted: 0 when asked or parked, else
    /// the earliest `wake_at`, else NO_WAKE. Reading clears it.
    pub fn fall(&mut self) -> f64 {
        let asked = core::mem::take(&mut self.asked) || self.parked_len > 0;
        let at = core::mem::replace(&mut self.wake_at, f64::INFINITY);
        match () {
            _ if asked => 0.0,
            _ if at.is_finite() => (at - now()).max(0.0),
            _ => NO_WAKE,
        }
    }

    pub fn edge_at(&self) -> f64 {
        self.edge_at
    }

    pub fn quota_ms(&self) -> f64 {
        self.quota_ms
    }

    pub fn remaining(&self) -> f64 {
        (self.edge_at + self.quota_ms - now()).max(0.0)
    }

    pub fn spent(&self) -> bool {
        now() >= self.edge_at + self.quota_ms
    }

    pub fn ask_next(&mut self) {
        self.asked = true;
    }

    /// The earliest asked wins.
    pub fn wake_at(&mut self, at: f64) {
        self.wake_at = self.wake_at.min(at);
    }

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

pub fn yield_if_spent<const N: usize>(edge: fn() -> &'static mut Edge<N>) -> impl Future<Output = ()> {
    park_if(edge, Edge::spent)
}

pub fn next_turn<const N: usize>(edge: fn() -> &'static mut Edge<N>) -> impl Future<Output = ()> {
    park_if(edge, |_| true)
}

/// Parked until the next rise when `wait` says so. `edge` is a fn so no
/// borrow is held across the await.
fn park_if<const N: usize>(edge: fn() -> &'static mut Edge<N>, wait: fn(&Edge<N>) -> bool) -> impl Future<Output = ()> {
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
        Notify { waker: None, notified: false }
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
