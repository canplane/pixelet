// xpute-runtime/ipc/change.rs

//! The read half of the channel: which units of state changed, as data.
//!
//! A change is three numbers — a topic (u16, a kind of state the caller
//! numbers), a key (u64, the unit within it) and a revision (u32). A source
//! calls `bump`; a reader either compares a key's revision with the one it
//! remembers, or reads the journal from its own cursor. How a reader is woken
//! — polled from a tick, a listener, a React hook, a worker's mirror — is
//! built over this and is not part of it
//! (issue/20260913-the-read-direction-a-change-log.md).
//!
//! A revision is the topic's tick at the key's last change, one counter per
//! topic rather than per key, so a key forgotten and changed again gets a
//! revision it never had and a reader holding an old one cannot mistake the
//! new state for the one it saw — Bevy's change ticks, per topic. The tick is
//! u32 and a wrap is not handled: four billion changes to one topic.
//!
//! The journal is one fixed-capacity ring for every topic, laid out as lanes,
//! indexed by position modulo capacity. A cursor is a position; positions only
//! grow, and a cursor more than `capacity` behind the head has been
//! overwritten and is not readable — the reader re-reads the topics it cares
//! about and takes the head. Nothing queues per reader, so the log's memory is
//! fixed however many read it.
//!
//! A change carries no value. The value is the owner's, read by key where it
//! lives, so the journal stays small and a span of it crosses a boundary as a
//! frame body unchanged.
//!
//! A key is i64 in the lane, so a packed word past 2^63 is that word's bits
//! read as signed and always finds the same entry. A cursor is u32: the
//! position word the glue reads is one, and a wrap is four billion changes.

use std::collections::HashMap;

use fnv::FnvBuildHasher;

pub type Topic = u32;
pub type Revision = u32;
/// A position in the journal: how many changes had been written before it.
pub type Cursor = u32;

pub struct ChangeLogOptions {
    /// Topics are 0 .. topics - 1.
    pub topics: u32,
    /// Journal entries kept; a reader further behind than this is lost.
    pub capacity: u32,
    /// Keys a topic's table holds before it grows.
    pub keys_per_topic: Option<u32>,
}

/// What a reader walks: the journal's lanes and positions. ChangeLog is one;
/// a log kept in a WASM core and read through views is another.
pub trait ChangeJournal {
    fn topics(&self) -> u32;
    fn head(&self) -> Cursor;
    fn readable(&self, cursor: Cursor) -> bool;
    fn slot(&self, position: Cursor) -> u32;
    fn topic(&self) -> &[u16];
    fn key(&self) -> &[i64];
    fn revision(&self) -> &[u32];
}

pub struct ChangeLog {
    pub capacity: u32,

    /// Journal lanes. Entry at position p is at index `slot(p)`.
    pub topic: Box<[u16]>,
    pub key: Box<[i64]>,
    pub revision: Box<[u32]>,

    _head: Cursor,
    ticks: Box<[u32]>,
    /// Per topic: each live key's latest revision.
    revisions: Vec<HashMap<i64, u32, FnvBuildHasher>>,
}

impl ChangeLog {
    pub fn new(opts: ChangeLogOptions) -> ChangeLog {
        let topics = opts.topics;
        let capacity = opts.capacity;
        let keys_per_topic = opts.keys_per_topic.unwrap_or(64);
        if !(1..=0x10000).contains(&topics) {
            xpute_core::bug!(EINVAL, topics);
        }
        if capacity < 1 {
            xpute_core::bug!(EINVAL);
        }
        let topic = vec![0u16; capacity as usize].into_boxed_slice();
        let key = vec![0i64; capacity as usize].into_boxed_slice();
        let revision = vec![0u32; capacity as usize].into_boxed_slice();
        let ticks = vec![0u32; topics as usize].into_boxed_slice();
        let mut revisions = Vec::new();
        for _ in 0..topics {
            revisions.push(HashMap::with_capacity_and_hasher(keys_per_topic as usize, FnvBuildHasher::default()));
        }
        ChangeLog {
            capacity,
            topic,
            key,
            revision,
            _head: 0,
            ticks,
            revisions,
        }
    }

    pub fn topics(&self) -> u32 {
        self.ticks.len() as u32
    }

    // ---- source ----

    /// Records that `key` in `topic` changed. Returns its new revision.
    pub fn bump(&mut self, topic: Topic, key: i64) -> Revision {
        let tick = self.ticks[topic as usize].wrapping_add(1);
        self.ticks[topic as usize] = tick;

        self.revisions[topic as usize].insert(key, tick);

        let i = self.slot(self._head) as usize;
        self.topic[i] = topic as u16;
        self.key[i] = key;
        self.revision[i] = tick;
        self._head = self._head.wrapping_add(1);
        tick
    }

    /// Drops a key that has left — a page unloaded — from its topic's table.
    /// Its revision reads 0 afterward; the journal keeps what was written.
    pub fn forget(&mut self, topic: Topic, key: i64) {
        self.revisions[topic as usize].remove(&key);
    }

    // ---- reader: point ----

    /// The key's revision, or 0 when it has never changed or was forgotten.
    pub fn revision_of(&self, topic: Topic, key: i64) -> Revision {
        self.revisions[topic as usize].get(&key).copied().unwrap_or(0)
    }

    /// The topic's tick: moved means something in the topic changed.
    pub fn tick(&self, topic: Topic) -> Revision {
        self.ticks[topic as usize]
    }

    // ---- reader: journal ----

    /// The position the next change will be written at. A reader that has
    /// read up to here takes it as its cursor.
    pub fn head(&self) -> Cursor {
        self._head
    }

    /// Whether every entry from `cursor` to the head is still in the ring.
    pub fn readable(&self, cursor: Cursor) -> bool {
        cursor <= self._head && self._head - cursor <= self.capacity
    }

    /// Where the entry at a position lives in the lanes.
    pub fn slot(&self, position: Cursor) -> u32 {
        position % self.capacity
    }

    /// The lanes' addresses, for a reader through views (the glue's): topic,
    /// key, revision, the head word and the ticks.
    pub fn lanes(&self) -> [usize; 5] {
        [
            self.topic.as_ptr() as usize,
            self.key.as_ptr() as usize,
            self.revision.as_ptr() as usize,
            &self._head as *const u32 as usize,
            self.ticks.as_ptr() as usize,
        ]
    }
}

impl ChangeJournal for ChangeLog {
    fn topics(&self) -> u32 {
        ChangeLog::topics(self)
    }
    fn head(&self) -> Cursor {
        ChangeLog::head(self)
    }
    fn readable(&self, cursor: Cursor) -> bool {
        ChangeLog::readable(self, cursor)
    }
    fn slot(&self, position: Cursor) -> u32 {
        ChangeLog::slot(self, position)
    }
    fn topic(&self) -> &[u16] {
        &self.topic
    }
    fn key(&self) -> &[i64] {
        &self.key
    }
    fn revision(&self) -> &[u32] {
        &self.revision
    }
}

#[cfg(test)]
#[path = "change.test.rs"]
mod test;
