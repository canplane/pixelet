// xpute_core/ipc/change.rs

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
//! Keys are i64 in the lane, as the TypeScript's BigInt64Array; a key past
//! 2^63 is the same key folded. A cursor is a u32 here: the position word the
//! glue reads is one, and a wrap is four billion changes.

use crate::ds::arena::{ArenaOptions, Vector};
use crate::ds::map::{IndexedMap, I64_KEYS};
use crate::ds::typed_alloc::{TypedAlloc, HEAP_ALLOC};
use crate::kit::array::{AnyElement, I64Array, TypedArray, U32Array};

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
    pub alloc: Option<Box<dyn TypedAlloc>>,
}

/// What a reader walks: the journal's lanes and positions. ChangeLog is one;
/// a log kept in a WASM core and read through views is another.
pub trait ChangeJournal {
    fn topics(&self) -> u32;
    fn head(&self) -> Cursor;
    fn readable(&self, cursor: Cursor) -> bool;
    fn slot(&self, position: Cursor) -> u32;
    fn topic(&self) -> &TypedArray<u16>;
    fn key(&self) -> &I64Array;
    fn revision(&self) -> &U32Array;
}

pub struct ChangeLog {
    pub capacity: u32,

    /// Journal lanes. Entry at position p is at index `slot(p)`.
    pub topic: TypedArray<u16>,
    pub key: I64Array,
    pub revision: U32Array,

    _head: Cursor,
    ticks: U32Array,
    tables: Vec<IndexedMap<crate::ds::map::I64Keys>>,
    revisions: Vec<Vector<u32>>,
}

/// A `Box<dyn TypedAlloc>` shared by several structures, as one allocator
/// is passed to each there: each takes its views from the one pool.
struct Shared(std::rc::Rc<core::cell::RefCell<Box<dyn TypedAlloc>>>);

impl TypedAlloc for Shared {
    fn take_view(&mut self, kind: crate::kit::array::Kind, length: u32) -> crate::kit::array::ArrayBufferView {
        self.0.borrow_mut().take_view(kind, length)
    }
    fn give(&mut self, view: &crate::kit::array::ArrayBufferView) {
        self.0.borrow_mut().give(view)
    }
}

impl ChangeLog {
    pub fn new(opts: ChangeLogOptions) -> ChangeLog {
        let topics = opts.topics;
        let capacity = opts.capacity;
        let keys_per_topic = opts.keys_per_topic.unwrap_or(64);
        let alloc = Shared(std::rc::Rc::new(core::cell::RefCell::new(opts.alloc.unwrap_or_else(|| Box::new(HEAP_ALLOC)))));
        if !(1..=0x10000).contains(&topics) {
            panic!("change: {topics} topics, want 1..65536");
        }
        if capacity < 1 {
            panic!("change: a journal needs a capacity");
        }
        let mut a = Shared(alloc.0.clone());
        let topic = TypedArray::of(&a.take_view(u16::KIND, capacity)).unwrap();
        let key = TypedArray::of(&a.take_view(i64::KIND, capacity)).unwrap();
        let revision = TypedArray::of(&a.take_view(u32::KIND, capacity)).unwrap();
        let ticks = TypedArray::of(&a.take_view(u32::KIND, topics)).unwrap();
        let mut tables = Vec::new();
        let mut revisions = Vec::new();
        for _ in 0..topics {
            tables.push(IndexedMap::new(I64_KEYS, keys_per_topic, Some(Box::new(Shared(alloc.0.clone())))));
            revisions.push(Vector::new(ArenaOptions {
                init_cap: Some(keys_per_topic),
                alloc: Some(Box::new(Shared(alloc.0.clone()))),
                ..Default::default()
            }));
        }
        ChangeLog {
            capacity,
            topic,
            key,
            revision,
            _head: 0,
            ticks,
            tables,
            revisions,
        }
    }

    pub fn topics(&self) -> u32 {
        self.ticks.length() as u32
    }

    // ---- source ----

    /// Records that `key` in `topic` changed. Returns its new revision.
    pub fn bump(&mut self, topic: Topic, key: i64) -> Revision {
        let tick = self.ticks[topic as usize].wrapping_add(1);
        self.ticks.put(topic as usize, tick);

        let pos = self.tables[topic as usize].insert(key);
        let revs = &mut self.revisions[topic as usize];
        if pos == revs.len() {
            revs.push(tick).expect("change: a revision lane past its cap");
        } else {
            revs.set(pos, tick).expect("change: a revision lane past its cap");
        }

        let i = self.slot(self._head) as usize;
        self.topic.put(i, topic as u16);
        self.key.put(i, key);
        self.revision.put(i, tick);
        self._head = self._head.wrapping_add(1);
        tick
    }

    /// Drops a key that has left — a page unloaded — from its topic's table.
    /// Its revision reads 0 afterwards; the journal keeps what was written.
    pub fn forget(&mut self, topic: Topic, key: i64) {
        let table = &mut self.tables[topic as usize];
        let pos = table.find(key);
        if pos < 0 {
            return;
        }
        let pos = pos as u32;
        let from = table.remove_at(pos);
        let revs = &mut self.revisions[topic as usize];
        if from != pos {
            let moved = revs.get(from);
            revs.set(pos, moved).expect("change: a revision lane past its cap");
        }
        revs.truncate(from);
    }

    // ---- reader: point ----

    /// The key's revision, or 0 when it has never changed or was forgotten.
    pub fn revision_of(&self, topic: Topic, key: i64) -> Revision {
        let pos = self.tables[topic as usize].find(key);
        if pos < 0 {
            0
        } else {
            self.revisions[topic as usize].get(pos as u32)
        }
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
            self.topic.as_slice().as_ptr() as usize,
            self.key.as_slice().as_ptr() as usize,
            self.revision.as_slice().as_ptr() as usize,
            &self._head as *const u32 as usize,
            self.ticks.as_slice().as_ptr() as usize,
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
    fn topic(&self) -> &TypedArray<u16> {
        &self.topic
    }
    fn key(&self) -> &I64Array {
        &self.key
    }
    fn revision(&self) -> &U32Array {
        &self.revision
    }
}

#[cfg(test)]
#[path = "change.test.rs"]
mod test;
