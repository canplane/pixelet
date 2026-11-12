// xpute-core/resource.rs

//! A handle names an object on the side that serves it, which issued it
//! (link.rs). Handles come from a counter from 1, the root being 0, and are
//! never used again, so a frame late for a released object reaches no other;
//! issuing stops as a bug past the largest a command holds.

use std::collections::BTreeMap;

use crate::io::frame::HANDLE_MAX;

pub type Handle = u32;

/// The objects a side serves, by handle.
pub struct Table<T> {
    map: BTreeMap<Handle, T>,
    next: Handle,
}

impl<T> Table<T> {
    pub const fn new() -> Table<T> {
        Table {
            map: BTreeMap::new(),
            next: 1,
        }
    }

    /// Issues the next handle, for `put` to fill.
    pub fn issue(&mut self) -> Handle {
        let h = self.next;
        xpute_kit::ensure!(h <= HANDLE_MAX, EMFILE, h);
        self.next = h + 1;
        h
    }

    pub fn put(&mut self, h: Handle, v: T) {
        self.map.insert(h, v);
    }

    /// Puts `v` at the next handle, and answers it.
    pub fn add(&mut self, v: T) -> Handle {
        let h = self.issue();
        self.put(h, v);
        h
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        self.map.get(&h)
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        self.map.get_mut(&h)
    }

    pub fn remove(&mut self, h: Handle) -> Option<T> {
        self.map.remove(&h)
    }
}
