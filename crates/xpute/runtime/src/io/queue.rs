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
//! - **yield:** `sync` takes a category's *entire* current wishlist, and what
//!   it leaves out stops being wanted there and then. One not yet started is
//!   simply gone; one already on the wire is told to abort, and the wishlist
//!   does not wait for it to answer.
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
//! category and a priority. The wishlist holds N demands and never grows; a
//! demand past them is refused, and its category asks again next turn.
//!
//! What is wanted and what is on the wire are two sets, not one set with a
//! state on each member, because they have different lifetimes: a wishlist is
//! replaced wholesale whenever the guest's mind changes, and a request lives
//! until whoever is running it says it is over. Held as one, the shorter
//! lifetime cannot end without stranding the longer, and the queue either
//! loses count of what is out or keeps what nobody wants.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand {
    /// Stable identity across syncs. Two demands sharing a key are the same
    /// work: re-submitting one already wanted updates its priority, one
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

/// A demand the guest wants run. Only `sync` puts one here and only `sync`
/// takes one away, so what is here is what the wishlists say and nothing
/// else — there is no way to hold a want nobody asked for.
#[derive(Clone, Copy, Debug, Default)]
struct Want {
    demand: Demand,
    enqueue_ms: f64,
}

/// A demand the outside was told to start and has not settled. Only `pump`
/// makes one and only `settle` unmakes one, so what is here is what is on
/// the wire — and the credits are its length rather than a number kept
/// beside it.
///
/// `stop_ms` is when it was told to stop, which is a thing that happened to
/// the request and not a state of the demand: a request told to stop is
/// still on the wire and still spending its credit until whoever runs it
/// says it is over. It is what makes an abort told exactly once without
/// anything having to remember that it was.
#[derive(Clone, Copy, Debug, Default)]
struct Flight {
    demand: Demand,
    enqueue_ms: f64,
    start_ms: f64,
    stop_ms: Option<f64>,
}

/// The wants and the flights, and the two outboxes the outside drains.
///
/// They are two sets and not one with a state on it, and every invariant
/// below is that separation rather than a rule anything has to keep:
///
/// - **A want is wanted.** `sync` writes the wishlist and nothing else does,
///   so a demand no category asked for cannot be among the wants, and cannot
///   take the room a wishlist needs. A wishlist replaced wholesale — a
///   zoom's is — meets only other categories' wants, never the leavings of
///   its own.
/// - **A flight is on the wire.** It is made where the outside is told to
///   start and unmade where the outside settles, so the credits, which are
///   its length, are what is actually out. Nothing can drop a flight to free
///   room, because room is the wants' and a flight is not one.
/// - **A flight outliving its want is the ordinary case, not a state.** The
///   two sets simply stop overlapping. Nothing has to be marked, and nothing
///   has to be swept, and a wishlist never waits on one.
/// - **An abort buys a credit and nothing else.** It is not how a demand
///   stops being wanted — `sync` does that, and it has already happened by
///   the time anything is told. So nothing is aborted while there are
///   credits to spare: a request nobody wants any more is left to finish,
///   and what it brings back is the cache's to keep. Only where a demand
///   has no credit and a flight has one nobody wants is the trade made.
pub struct IoQueue<const N: usize> {
    want: [Want; N],
    want_len: usize,
    flight: [Flight; N],
    flight_len: usize,
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
            want: [Want::default(); N],
            want_len: 0,
            flight: [Flight::default(); N],
            flight_len: 0,
            started: [Demand::default(); N],
            started_len: 0,
            aborted: [0; N],
            aborted_len: 0,
            revision: 0,
        }
    }

    fn want_at(&self, key: u64) -> Option<usize> {
        (0..self.want_len).find(|&k| self.want[k].demand.key == key)
    }

    fn flight_at(&self, key: u64) -> Option<usize> {
        (0..self.flight_len).find(|&k| self.flight[k].demand.key == key)
    }

    /// Moves after every change to either set.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    pub fn counts(&self) -> IoCounts {
        IoCounts {
            queued: (0..self.want_len).filter(|&k| self.flight_at(self.want[k].demand.key).is_none()).count() as u32,
            running: self.flight_len as u32,
        }
    }

    /// Wanted or in flight, of one category.
    pub fn pending_count(&self, category: u32) -> u32 {
        let wanted = self.want[..self.want_len].iter().filter(|w| w.demand.category == category).count();
        let flying_unwanted = self.flight[..self.flight_len]
            .iter()
            .filter(|f| f.demand.category == category && self.want_at(f.demand.key).is_none())
            .count();
        (wanted + flying_unwanted) as u32
    }

    /// Whether a demand with this key is on the wire.
    pub fn running(&self, key: u64) -> bool {
        self.flight_at(key).is_some()
    }

    /// Whether this queue still has anything to do with the key: wanted, on
    /// the wire, or both. It is what a name for the key has to outlive, and
    /// deliberately not two questions — a caller that asked only one of them
    /// would be asking whether a demand is in a particular set, which is the
    /// queue's business and changes under it.
    pub fn knows(&self, key: u64) -> bool {
        self.want_at(key).is_some() || self.flight_at(key).is_some()
    }

    /// Replaces one category's wishlist; other categories are untouched. A
    /// key already wanted keeps the time it was first asked for and takes the
    /// new priority; one already in flight is left flying.
    ///
    /// Nothing is aborted here. A demand stops being wanted by not being on
    /// the list, which is done and true the moment this returns, whatever the
    /// wire is doing; who is told to stop, and whether anyone is, is a
    /// question about credits and `pump` answers it.
    ///
    /// Returns how many of `wanted` found no room — the room is the wants',
    /// and what fills it is other categories' wants alone.
    pub fn sync(&mut self, category: u32, wanted: &[Demand]) -> u32 {
        // The category's wants are replaced whole. Backwards, since a removal
        // moves the last want into the hole.
        let mut kept: [(u64, f64); N] = [(0, 0.0); N];
        let mut kept_len = 0;
        for at in (0..self.want_len).rev() {
            if self.want[at].demand.category != category {
                continue;
            }
            if kept_len < N {
                kept[kept_len] = (self.want[at].demand.key, self.want[at].enqueue_ms);
                kept_len += 1;
            }
            self.want_len -= 1;
            self.want[at] = self.want[self.want_len];
        }

        let at_first = now();
        let mut refused = 0;
        for demand in wanted {
            if self.want_len == N {
                refused += 1;
                continue;
            }
            let enqueue_ms = kept[..kept_len].iter().find(|(k, _)| *k == demand.key).map_or(at_first, |(_, t)| *t);
            self.want[self.want_len] = Want { demand: *demand, enqueue_ms };
            self.want_len += 1;
        }
        self.revision += 1;
        refused
    }

    /// Starts wants that are not yet on the wire, lowest priority first,
    /// until `credits` are flying: the turn's end, once every category has
    /// synced.
    ///
    /// N is a ceiling on the credits as well as on the wishlist. A grant past
    /// it is not an error to report but a number the queue cannot mean, since
    /// what is on the wire is a set it holds.
    pub fn pump(&mut self, credits: u32) {
        let credits = credits.min(N as u32);
        let mut moved = false;
        // Small N: a scan per start is cheaper to read than a heap, and at
        // most `credits` starts happen a turn. Ties go to the earlier want.
        while (self.flight_len as u32) < credits && self.started_len < N {
            let Some(at) = self.next_want() else { break };
            let w = self.want[at];
            self.flight[self.flight_len] = Flight {
                demand: w.demand,
                enqueue_ms: w.enqueue_ms,
                start_ms: now(),
                stop_ms: None,
            };
            self.flight_len += 1;
            self.started[self.started_len] = w.demand;
            self.started_len += 1;
            moved = true;
        }
        moved |= self.reclaim(credits);
        if moved {
            self.revision += 1;
        }
    }

    /// The want that should run next: the lowest priority number not already
    /// on the wire.
    fn next_want(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for k in 0..self.want_len {
            if self.flight_at(self.want[k].demand.key).is_some() {
                continue;
            }
            if best.is_none_or(|b| self.want[k].demand.priority < self.want[b].demand.priority) {
                best = Some(k);
            }
        }
        best
    }

    /// Tells as many flights nobody wants to stop as there are wants with no
    /// credit to run on. This is the whole of what an abort is for: without
    /// it a wishlist the guest replaced wholesale would wait on the last one
    /// to drain, and the credits would belong to whatever asked first rather
    /// than to what is wanted now.
    ///
    /// Only what nobody wants, and only as many as are short, so a request
    /// that is merely obsolete is left to finish and answer — the answer is
    /// worth keeping even when the question has moved on. Newest started
    /// first among them, since it has done the least and so loses the least;
    /// an older one is likelier to be a moment from answering. (When a
    /// transport can say a response has begun, that is the better measure and
    /// belongs here.)
    ///
    /// A credit does not come back here. The request goes on spending it
    /// until whoever runs it says it is over, which is why nothing built on
    /// top may depend on an abort landing: it makes the credit come back
    /// sooner and nothing else.
    fn reclaim(&mut self, credits: u32) -> bool {
        let mut short = 0;
        for k in 0..self.want_len {
            if self.flight_at(self.want[k].demand.key).is_none() {
                short += 1;
            }
        }
        // A flight already told to stop is a credit on its way back, so it
        // counts against the shortfall like a free one. Without that, a turn
        // that is still short tells the next flight, and the next, until
        // nothing obsolete is left running — which is the thing this is
        // meant not to do.
        let coming = self.flight[..self.flight_len].iter().filter(|f| f.stop_ms.is_some()).count() as u32;
        let free = credits.saturating_sub(self.flight_len as u32) + coming;
        let short = short.min(credits).saturating_sub(free);
        let mut told = false;
        for _ in 0..short {
            let mut newest: Option<usize> = None;
            for k in 0..self.flight_len {
                let f = self.flight[k];
                if f.stop_ms.is_some() || self.want_at(f.demand.key).is_some() {
                    continue;
                }
                if newest.is_none_or(|b| f.start_ms > self.flight[b].start_ms) {
                    newest = Some(k);
                }
            }
            let Some(at) = newest else { break };
            self.flight[at].stop_ms = Some(now());
            if self.aborted_len < N {
                self.aborted[self.aborted_len] = self.flight[at].demand.key;
                self.aborted_len += 1;
            }
            told = true;
        }
        told
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

    /// The outside's report that a started demand finished, however it did —
    /// answered, failed or cancelled. None where the key is not on the wire,
    /// which a settle arriving twice, or after a reset, is.
    ///
    /// The demand is over, so it is neither on the wire nor wanted: an answer
    /// the guest has not read yet is still an answer, and a wishlist that
    /// asks again next turn is how it asks for another.
    pub fn settle(&mut self, key: u64) -> Option<Settled> {
        let at = self.flight_at(key)?;
        let f = self.flight[at];
        self.flight_len -= 1;
        self.flight[at] = self.flight[self.flight_len];
        if let Some(at) = self.want_at(key) {
            self.want_len -= 1;
            self.want[at] = self.want[self.want_len];
        }
        self.revision += 1;
        Some(Settled {
            demand: f.demand,
            enqueue_ms: f.enqueue_ms,
            start_ms: f.start_ms,
            end_ms: now(),
        })
    }

    /// Aborts everything on the wire and forgets both sets.
    pub fn reset(&mut self) {
        for k in 0..self.flight_len {
            if self.flight[k].stop_ms.is_none() && self.aborted_len < N {
                self.aborted[self.aborted_len] = self.flight[k].demand.key;
                self.aborted_len += 1;
            }
        }
        self.want_len = 0;
        self.flight_len = 0;
        self.revision += 1;
    }
}

#[cfg(test)]
#[path = "queue.test.rs"]
mod test;
