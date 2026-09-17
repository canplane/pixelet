// pxl_core/module/world/page_table.rs

//! The pages resident in the client, keyed by L3 page index, as a table.
//!
//! The key index is an IndexedMap — dense positions, an open-addressing
//! table over packed page keys — and what a page holds sits at its position
//! in a handle array: for terrain the views over the page's pool blocks, for
//! POI the page's records. The keys, the positions and the walk are linear.
//!
//! A table given a world topic writes every set and clear to the world
//! change log (module/world/change.rs) — which page, as its own key — and its
//! `revision` is that topic's tick; readers compare it. Without a topic it
//! only counts. The keys are normalised through TO_XY on the way in (xy.rs on
//! why), and come back out unsigned, the form every IVec is.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::ds::map::{I64Keys, IndexedMap};

use crate::module::world::change::{bump_world, world_changes, WorldTopic};

pub struct PageTable<T> {
    index: IndexedMap<I64Keys>,
    /// What each position holds.
    pub handles: Vec<T>,
    count: u32,
    pub topic: Option<WorldTopic>,
}

impl<T> PageTable<T> {
    pub fn new(topic: Option<WorldTopic>) -> PageTable<T> {
        PageTable {
            index: IndexedMap::new(I64Keys, 64),
            handles: Vec::new(),
            count: 0,
            topic,
        }
    }

    /// Moves on every set and clear: the topic's tick, or a count.
    pub fn revision(&self) -> u32 {
        match self.topic {
            None => self.count,
            Some(topic) => world_changes().tick(topic),
        }
    }

    pub fn size(&self) -> u32 {
        self.index.size()
    }

    pub fn get(&self, page_idx: IVec) -> Option<&T> {
        let pos = self.index.find(TO_XY(page_idx) as i64);
        if pos < 0 {
            None
        } else {
            self.handles.get(pos as usize)
        }
    }

    pub fn get_mut(&mut self, page_idx: IVec) -> Option<&mut T> {
        let pos = self.index.find(TO_XY(page_idx) as i64);
        if pos < 0 {
            None
        } else {
            self.handles.get_mut(pos as usize)
        }
    }

    pub fn has(&self, page_idx: IVec) -> bool {
        self.index.find(TO_XY(page_idx) as i64) >= 0
    }

    pub fn set(&mut self, page_idx: IVec, value: T) {
        let key = TO_XY(page_idx);
        let pos = self.index.insert(key as i64) as usize;
        if pos < self.handles.len() {
            self.handles[pos] = value;
        } else {
            self.handles.push(value);
        }
        self.changed(key, false);
    }

    /// Removes the page and returns what it held, or None.
    pub fn clear(&mut self, page_idx: IVec) -> Option<T> {
        let key = TO_XY(page_idx);
        let pos = self.index.find(key as i64);
        if pos < 0 {
            return None;
        }
        let from = self.index.remove_at(pos as u32) as usize;
        // The last position moved into the removed one's place.
        let held = self.handles.swap_remove(pos as usize);
        debug_assert_eq!(self.handles.len(), from);
        self.changed(key, true);
        Some(held)
    }

    /// The page key at a position, as the unsigned IVec it was set with.
    pub fn key_at(&self, pos: u32) -> IVec {
        self.index.key_at(pos) as u64
    }

    pub fn keys(&self) -> impl Iterator<Item = IVec> + '_ {
        (0..self.index.size()).map(|pos| self.key_at(pos))
    }

    pub fn entries(&self) -> impl Iterator<Item = (IVec, &T)> + '_ {
        (0..self.index.size()).map(|pos| (self.key_at(pos), &self.handles[pos as usize]))
    }

    fn changed(&mut self, key: IVec, left: bool) {
        self.count += 1;
        let Some(topic) = self.topic else { return };
        bump_world(topic, key);
        if left {
            world_changes().forget(topic, key as i64);
        }
    }
}

#[cfg(test)]
#[path = "page_table.test.rs"]
mod test;
