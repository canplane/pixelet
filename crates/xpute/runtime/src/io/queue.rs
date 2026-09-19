// xpute-runtime/io/queue.rs

//! A guest's I/O queue: what it wants run, in the order it wants it.
//!
//! The turn it shares with the tick (sched/tick.rs) is the only thing the two
//! have in common: a tick spends a quota of time on the guest's own work,
//! this spends a grant of credits on what the host runs for it.
//!
//! The guest does no I/O. It says what it wants and in what order, and the
//! central scheduler — which owns the connections, the way an OS owns its
//! devices — runs what the queue starts, within the credits it grants: the
//! pump starts no more than that many at once. What was started and what was
//! aborted wait here for the outside to take, and the outside settles each.
//!
//! Two levers, no feedback control:
//!
//! - **quota:** at most the granted credits run at once, across every
//!   category, ordered by priority — a near page of one category beats a far
//!   page of another.
//! - **yield:** `sync` takes a category's *entire* current wishlist. Anything
//!   of that category queued or in flight that is not on it is dropped
//!   (queued) or aborted (in flight), freeing its slot for what is wanted now.
//!
//! **One pump a turn, after every category has submitted.** With a pump per
//! submission, whichever category synced first won outright: nine demands of
//! one filled every slot before another had submitted anything, and a page
//! at priority 1 sat 358 ms behind pages at priority 2. Priority can only
//! order the queue; it cannot preempt a request already in flight, so
//! submission order was silently beating it. The diff half of `sync` stays
//! immediate on purpose — dropping and aborting must not wait.
//!
//! A demand is a key the guest chose (what it names is the guest's), a
//! category and a priority. The queue holds N rows and never grows; a demand
//! past them is refused, and its category asks again next turn.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    /// Stable identity across syncs. Two demands sharing a key are the same
    /// work: re-submitting one already queued updates its priority, one
    /// already in flight is left alone.
    pub key: u64,
    /// Scopes `sync`'s diff.
    pub category: u32,
    /// Lower runs sooner.
    pub priority: f64,
}

/// A demand that finished, and when it was enqueued, started and ended.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settled {
    pub demand: Demand,
    pub enqueue_ms: f64,
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IoCounts {
    pub queued: u32,
    pub running: u32,
}

const QUEUED: u8 = 1;
const RUNNING: u8 = 2;

#[derive(Clone, Copy, Debug, Default)]
struct Row {
    demand: Demand,
    state: u8,
    abort_sent: bool,
    enqueue_ms: f64,
    start_ms: f64,
}

pub struct IoQueue<const N: usize> {
    rows: [Row; N],
    len: usize,
    started: [Demand; N],
    started_len: usize,
    aborted: [u64; N],
    aborted_len: usize,
    revision: u32,
}

impl<const N: usize> Default for IoQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> IoQueue<N> {
    pub fn new() -> Self {
        IoQueue {
            rows: [Row::default(); N],
            len: 0,
            started: [Demand::default(); N],
            started_len: 0,
            aborted: [0; N],
            aborted_len: 0,
            revision: 0,
        }
    }

    fn find(&self, key: u64) -> Option<usize> {
        (0..self.len).find(|&k| self.rows[k].demand.key == key)
    }

    fn remove(&mut self, at: usize) {
        self.len -= 1;
        self.rows[at] = self.rows[self.len];
    }

    fn count(&self, state: u8) -> u32 {
        self.rows[..self.len].iter().filter(|r| r.state == state).count() as u32
    }

    /// Moves after every queue or in-flight change.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    pub fn counts(&self) -> IoCounts {
        IoCounts {
            queued: self.count(QUEUED),
            running: self.count(RUNNING),
        }
    }

    /// Queued and in-flight demands of one category.
    pub fn pending_count(&self, category: u32) -> u32 {
        self.rows[..self.len].iter().filter(|r| r.demand.category == category).count() as u32
    }

    /// Whether a demand with this key is in flight.
    pub fn running(&self, key: u64) -> bool {
        self.find(key).is_some_and(|at| self.rows[at].state == RUNNING)
    }

    /// Whether a demand with this key is queued or in flight.
    pub fn holds(&self, key: u64) -> bool {
        self.find(key).is_some()
    }

    /// Replaces one category's wishlist; other categories are untouched. New
    /// keys are queued, keys already queued take the new priority, keys in
    /// flight keep running; anything of the category not in `wanted` is
    /// dropped or aborted. Returns how many new demands found no row.
    pub fn sync(&mut self, category: u32, wanted: &[Demand]) -> u32 {
        // Backwards, since a removal moves the last row into the hole.
        for at in (0..self.len).rev() {
            let row = self.rows[at];
            if row.demand.category != category || wanted.iter().any(|d| d.key == row.demand.key) {
                continue;
            }
            if row.state == QUEUED {
                self.remove(at);
            } else if !row.abort_sent && self.aborted_len < N {
                self.aborted[self.aborted_len] = row.demand.key;
                self.aborted_len += 1;
                self.rows[at].abort_sent = true;
            }
        }

        let mut refused = 0;
        for demand in wanted {
            match self.find(demand.key) {
                Some(at) if self.rows[at].state == QUEUED => self.rows[at].demand = *demand,
                Some(_) => {}
                None if self.len < N => {
                    self.rows[self.len] = Row {
                        demand: *demand,
                        state: QUEUED,
                        abort_sent: false,
                        enqueue_ms: now(),
                        start_ms: -1.0,
                    };
                    self.len += 1;
                }
                None => refused += 1,
            }
        }
        self.revision += 1;
        refused
    }

    /// Starts queued demands, lowest priority first, until `credits` are in
    /// flight: the turn's end, once every category has synced.
    pub fn pump(&mut self, credits: u32) {
        let mut inflight = self.count(RUNNING);
        let mut started = false;
        // Small N: a scan per start is cheaper to read than a heap, and at most
        // `credits` starts happen a turn. Ties go to the earlier row.
        while inflight < credits && self.started_len < N {
            let mut best: Option<usize> = None;
            for k in 0..self.len {
                if self.rows[k].state == QUEUED && best.is_none_or(|b| self.rows[k].demand.priority < self.rows[b].demand.priority) {
                    best = Some(k);
                }
            }
            let Some(at) = best else { break };
            let row = &mut self.rows[at];
            row.state = RUNNING;
            row.start_ms = now();
            self.started[self.started_len] = row.demand;
            self.started_len += 1;
            inflight += 1;
            started = true;
        }
        if started {
            self.revision += 1;
        }
    }

    /// The demands started since the last clear, for the outside to run.
    pub fn started(&self) -> &[Demand] {
        &self.started[..self.started_len]
    }

    pub fn clear_started(&mut self) {
        self.started_len = 0;
    }

    /// The keys aborted since the last clear, for the outside to cancel; each
    /// is settled by the outside like any other.
    pub fn aborted(&self) -> &[u64] {
        &self.aborted[..self.aborted_len]
    }

    pub fn clear_aborted(&mut self) {
        self.aborted_len = 0;
    }

    /// The outside's report that a started demand finished, however it did.
    /// None when the key is not in flight — a reset forgets and aborts in one
    /// step, so a settle can arrive for a row already gone.
    pub fn settle(&mut self, key: u64) -> Option<Settled> {
        let at = self.find(key)?;
        let row = self.rows[at];
        if row.state != RUNNING {
            return None;
        }
        self.remove(at);
        self.revision += 1;
        Some(Settled {
            demand: row.demand,
            enqueue_ms: row.enqueue_ms,
            start_ms: row.start_ms,
            end_ms: now(),
        })
    }

    /// Aborts everything in flight and forgets the queue.
    pub fn reset(&mut self) {
        for at in 0..self.len {
            let row = self.rows[at];
            if row.state == RUNNING && !row.abort_sent && self.aborted_len < N {
                self.aborted[self.aborted_len] = row.demand.key;
                self.aborted_len += 1;
            }
        }
        self.len = 0;
        self.revision += 1;
    }
}

#[cfg(test)]
#[path = "queue.test.rs"]
mod test;
