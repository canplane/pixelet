// xpute_core/ipc/change_waker.rs

//! Push delivery over a change log: listeners per (topic, key), woken when a
//! pump finds their key in the journal.
//!
//! An adapter, not the protocol. The log never calls anyone; whoever owns the
//! pump decides when listeners run — once a tick, say — so a burst of changes
//! to one key wakes its listeners once, and a source's write never runs
//! consumer code inside it. A pump that finds its cursor overwritten wakes
//! every listener it holds: each of them re-reads, which is what `lost`
//! means.
//!
//! A topic listener hears every changed key of its topic instead, once per
//! key per pump, and null when the pump was lost — its filter is its own.
//!
//! The TypeScript's `subscribe` returns the unsubscribe closure; a closure
//! cannot hold the waker mutably here, so it returns a `Subscription` that
//! `unsubscribe` takes. The waker holds the journal with whoever writes it.

use core::cell::RefCell;
use std::rc::Rc;

use crate::ds::arena::{ArenaOptions, Vector};
use crate::ds::map::{I64Keys, IndexedMap, I64_KEYS};

use super::change::{ChangeJournal, Cursor, Topic};

type Listener = Box<dyn FnMut()>;
/// A changed key, or null: the pump lost its place, re-read the topic.
pub type TopicListener = Box<dyn FnMut(Option<i64>)>;

/// What `subscribe` hands back, for `unsubscribe`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subscription {
    topic: Topic,
    key: Option<i64>,
    id: u32,
}

pub struct ChangeWaker<J: ChangeJournal> {
    log: Rc<RefCell<J>>,
    cursor: Cursor,
    pumps: u32,
    /// Per topic: the keys listened to, their listeners, and the pump that
    /// last woke each, so a key bumped twice in one span wakes once.
    keys: Vec<IndexedMap<I64Keys>>,
    lists: Vec<Vec<Vec<(u32, Listener)>>>,
    woken: Vec<Vector<u32>>,
    /// Per topic: whole-topic listeners, and the keys already given to them
    /// this pump.
    topic_lists: Vec<Vec<(u32, TopicListener)>>,
    topic_seen: Vec<IndexedMap<I64Keys>>,
    next_id: u32,
}

impl<J: ChangeJournal> ChangeWaker<J> {
    pub fn new(log: Rc<RefCell<J>>) -> ChangeWaker<J> {
        let (cursor, topics) = {
            let l = log.borrow();
            (l.head(), l.topics())
        };
        let mut waker = ChangeWaker {
            log,
            cursor,
            pumps: 0,
            keys: Vec::new(),
            lists: Vec::new(),
            woken: Vec::new(),
            topic_lists: Vec::new(),
            topic_seen: Vec::new(),
            next_id: 0,
        };
        for _ in 0..topics {
            waker.keys.push(IndexedMap::new(I64_KEYS, 16));
            waker.lists.push(Vec::new());
            waker.woken.push(Vector::new(ArenaOptions {
                init_cap: Some(16),
                ..Default::default()
            }));
            waker.topic_lists.push(Vec::new());
            waker.topic_seen.push(IndexedMap::new(I64_KEYS, 64));
        }
        waker
    }

    pub fn subscribe(&mut self, topic: Topic, key: i64, listener: Listener) -> Subscription {
        let t = topic as usize;
        let pos = self.keys[t].insert(key) as usize;
        if pos == self.woken[t].len() as usize {
            self.woken[t].push(0).expect("waker: a woken lane past its cap");
        }
        if self.lists[t].len() <= pos {
            self.lists[t].resize_with(pos + 1, Vec::new);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.lists[t][pos].push((id, listener));
        Subscription { topic, key: Some(key), id }
    }

    /// Hears every key of `topic` that changes.
    pub fn subscribe_topic(&mut self, topic: Topic, listener: TopicListener) -> Subscription {
        let id = self.next_id;
        self.next_id += 1;
        self.topic_lists[topic as usize].push((id, listener));
        Subscription { topic, key: None, id }
    }

    /// The closure `subscribe` returns there.
    pub fn unsubscribe(&mut self, sub: Subscription) {
        let t = sub.topic as usize;
        let Some(key) = sub.key else {
            self.topic_lists[t].retain(|(id, _)| *id != sub.id);
            return;
        };
        let p = self.keys[t].find(key);
        if p < 0 {
            return;
        }
        let p = p as usize;
        self.lists[t][p].retain(|(id, _)| *id != sub.id);
        if !self.lists[t][p].is_empty() {
            return;
        }
        let from = self.keys[t].remove_at(p as u32) as usize;
        if from != p {
            let moved = self.lists[t].swap_remove(from);
            self.lists[t][p] = moved;
            let w = self.woken[t].get(from as u32);
            self.woken[t].set(p as u32, w).expect("waker: a woken lane past its cap");
        } else {
            self.lists[t].truncate(from);
        }
        self.woken[t].truncate(from as u32);
    }

    /// Wakes the listeners of every key changed since the last pump. Returns
    /// how many listeners ran.
    pub fn pump(&mut self) -> u32 {
        let head = self.log.borrow().head();
        if self.cursor == head {
            return 0;
        }
        self.pumps += 1;
        let pump = self.pumps;
        let mut ran = 0;
        let readable = self.log.borrow().readable(self.cursor);
        if !readable {
            for t in 0..self.lists.len() {
                for p in 0..self.keys[t].size() {
                    ran += self.wake(t as Topic, p, pump);
                }
                for (_, listener) in self.topic_lists[t].iter_mut() {
                    listener(None);
                    ran += 1;
                }
            }
        } else {
            for seen in self.topic_seen.iter_mut() {
                seen.clear();
            }
            for c in self.cursor..head {
                let (t, key) = {
                    let log = self.log.borrow();
                    let i = log.slot(c) as usize;
                    (log.topic()[i] as usize, log.key()[i])
                };
                let p = self.keys[t].find(key);
                if p >= 0 {
                    ran += self.wake(t as Topic, p as u32, pump);
                }
                if self.topic_lists[t].is_empty() {
                    continue;
                }
                let seen = &mut self.topic_seen[t];
                if seen.find(key) >= 0 {
                    continue;
                }
                seen.insert(key);
                for (_, listener) in self.topic_lists[t].iter_mut() {
                    listener(Some(key));
                    ran += 1;
                }
            }
        }
        self.cursor = head;
        ran
    }

    fn wake(&mut self, topic: Topic, pos: u32, pump: u32) -> u32 {
        let woken = &mut self.woken[topic as usize];
        if woken.get(pos) == pump {
            return 0;
        }
        woken.set(pos, pump).expect("waker: a woken lane past its cap");
        // A listener may unsubscribe as it runs; the TypeScript walks a copy
        // of the list. Here the list is borrowed for the walk, and an
        // unsubscribe is a call on the waker after it.
        let list = &mut self.lists[topic as usize][pos as usize];
        for (_, listener) in list.iter_mut() {
            listener();
        }
        list.len() as u32
    }
}
