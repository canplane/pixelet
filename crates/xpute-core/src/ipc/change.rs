//! The read half of the channel: which units of state changed —
//! @xpute/core/ipc/change.ts's change log, in the core, where the state
//! that changes most (the object store) now is.
//!
//! A change is a topic (u16), a key (u64) and a revision (u32): the topic's
//! tick at the key's last change, one counter per topic, so a key forgotten
//! and changed again gets a revision it never had. The journal is one
//! fixed-capacity ring for every topic, laid out as three lanes at fixed
//! addresses the glue reads as views; a position only grows, and a reader
//! more than `capacity` behind the head re-reads its topics. A change
//! carries no value: the value is read where it lives. Positions and ticks
//! are u32 and a wrap is not handled — four billion changes.

use std::collections::BTreeMap;

pub struct ChangeLog {
    ticks: Vec<u32>,
    revisions: Vec<BTreeMap<u64, u32>>,
    topic: Vec<u16>,
    key: Vec<u64>,
    revision: Vec<u32>,
    /// The position the next change is written at, and the ticks, as words
    /// the glue reads in place.
    head: u32,
}

impl ChangeLog {
    pub fn new(topics: usize, capacity: usize) -> ChangeLog {
        assert!((1..=0x10000).contains(&topics) && capacity >= 1, "change: a bad log shape");
        ChangeLog {
            ticks: vec![0; topics],
            revisions: vec![BTreeMap::new(); topics],
            topic: vec![0; capacity],
            key: vec![0; capacity],
            revision: vec![0; capacity],
            head: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.topic.len()
    }

    /// Records that `key` in `topic` changed; its new revision.
    pub fn bump(&mut self, topic: u16, key: u64) -> u32 {
        let t = topic as usize;
        let tick = self.ticks[t].wrapping_add(1);
        self.ticks[t] = tick;
        self.revisions[t].insert(key, tick);
        let i = self.head as usize % self.capacity();
        self.topic[i] = topic;
        self.key[i] = key;
        self.revision[i] = tick;
        self.head = self.head.wrapping_add(1);
        tick
    }

    /// Drops a key that has left from its topic's table; its revision reads
    /// 0 afterwards, and the journal keeps what was written.
    pub fn forget(&mut self, topic: u16, key: u64) {
        self.revisions[topic as usize].remove(&key);
    }

    pub fn revision_of(&self, topic: u16, key: u64) -> u32 {
        self.revisions[topic as usize].get(&key).copied().unwrap_or(0)
    }

    pub fn tick(&self, topic: u16) -> u32 {
        self.ticks[topic as usize]
    }

    /// The lanes' addresses, for the glue's views: topic, key, revision, the
    /// head word and the ticks.
    pub fn lanes(&self) -> [usize; 5] {
        [
            self.topic.as_ptr() as usize,
            self.key.as_ptr() as usize,
            self.revision.as_ptr() as usize,
            &self.head as *const u32 as usize,
            self.ticks.as_ptr() as usize,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bump_is_the_topics_tick_and_a_forgotten_key_reads_zero() {
        let mut log = ChangeLog::new(2, 4);
        assert_eq!(log.bump(0, 7), 1);
        assert_eq!(log.bump(0, 9), 2);
        assert_eq!(log.bump(1, 7), 1);
        assert_eq!(log.revision_of(0, 7), 1);
        assert_eq!(log.bump(0, 7), 3);
        log.forget(0, 7);
        assert_eq!(log.revision_of(0, 7), 0);
        assert_eq!(log.tick(0), 3);
        for k in 0..10 {
            log.bump(1, k);
        }
        assert_eq!(log.head, 14, "positions only grow past the capacity");
    }
}
