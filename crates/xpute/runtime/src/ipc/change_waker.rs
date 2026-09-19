// xpute-runtime/ipc/change_waker.rs

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
//! key per pump, and none when the pump was lost — its filter is its own.
//!
//! `subscribe` returns a `Subscription` that `unsubscribe` takes, since a
//! closure could not hold the waker mutably. The waker holds the journal
//! with whoever writes it.

use core::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use fnv::FnvBuildHasher;
use indexmap::IndexSet;

use super::change::{ChangeJournal, Cursor, Topic};

type Listener = Box<dyn FnMut()>;
/// A changed key, or none: the pump lost its place, re-read the topic.
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
    keys: Vec<IndexSet<i64, FnvBuildHasher>>,
    lists: Vec<Vec<Vec<(u32, Listener)>>>,
    woken: Vec<Vec<u32>>,
    /// Per topic: whole-topic listeners, and the keys already given to them
    /// this pump.
    topic_lists: Vec<Vec<(u32, TopicListener)>>,
    topic_seen: Vec<HashSet<i64, FnvBuildHasher>>,
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
            waker.keys.push(IndexSet::with_capacity_and_hasher(16, FnvBuildHasher::default()));
            waker.lists.push(Vec::new());
            waker.woken.push(Vec::with_capacity(16));
            waker.topic_lists.push(Vec::new());
            waker.topic_seen.push(HashSet::with_capacity_and_hasher(64, FnvBuildHasher::default()));
        }
        waker
    }

    pub fn subscribe(&mut self, topic: Topic, key: i64, listener: Listener) -> Subscription {
        let t = topic as usize;
        let (pos, _) = self.keys[t].insert_full(key);
        if pos == self.woken[t].len() {
            self.woken[t].push(0);
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
        let Some(p) = self.keys[t].get_index_of(&key) else { return };
        self.lists[t][p].retain(|(id, _)| *id != sub.id);
        if !self.lists[t][p].is_empty() {
            return;
        }
        // The last position moves into the hole; the lanes follow it.
        let from = self.keys[t].len() - 1;
        self.keys[t].swap_remove_index(p);
        if from != p {
            let moved = self.lists[t].swap_remove(from);
            self.lists[t][p] = moved;
            self.woken[t][p] = self.woken[t][from];
        } else {
            self.lists[t].truncate(from);
        }
        self.woken[t].truncate(from);
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
                for p in 0..self.keys[t].len() {
                    ran += self.wake(t as Topic, p as u32, pump);
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
                if let Some(p) = self.keys[t].get_index_of(&key) {
                    ran += self.wake(t as Topic, p as u32, pump);
                }
                if self.topic_lists[t].is_empty() || !self.topic_seen[t].insert(key) {
                    continue;
                }
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
        if woken[pos as usize] == pump {
            return 0;
        }
        woken[pos as usize] = pump;
        // The list is borrowed for the walk, so a listener cannot unsubscribe
        // as it runs: an unsubscribe is a call on the waker after it.
        let list = &mut self.lists[topic as usize][pos as usize];
        for (_, listener) in list.iter_mut() {
            listener();
        }
        list.len() as u32
    }
}
