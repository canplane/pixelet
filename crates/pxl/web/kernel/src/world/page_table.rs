// pxl-kernel/world/page_table.rs

//! The pages resident in the client, keyed by L3 page index, as a table.
//!
//! The key index is an IndexSet over packed page keys — dense positions,
//! swap-remove — and what a page holds sits at its position in a handle
//! array: for terrain the views over the page's blocks, for POI the page's
//! records. The keys, the positions and the walk are linear.
//!
//! Its `revision` counts every set and clear. The keys are normalised
//! through TO_XY on the way in (xy.rs on why), and come back out unsigned,
//! the form every IVec is.

use fnv::FnvBuildHasher;
use indexmap::IndexSet;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::xy::TO_XY;

pub struct PageTable<T> {
    index: IndexSet<IVec, FnvBuildHasher>,
    /// What each position holds.
    pub handles: Vec<T>,
    count: u32,
}

impl<T> Default for PageTable<T> {
    fn default() -> PageTable<T> {
        PageTable {
            index: IndexSet::with_capacity_and_hasher(64, FnvBuildHasher::default()),
            handles: Vec::new(),
            count: 0,
        }
    }
}

impl<T> PageTable<T> {
    /// Moves on every set and clear.
    pub fn revision(&self) -> u32 {
        self.count
    }

    pub fn size(&self) -> u32 {
        self.index.len() as u32
    }

    pub fn get(&self, page_idx: IVec) -> Option<&T> {
        let pos = self.index.get_index_of(&TO_XY(page_idx))?;
        self.handles.get(pos)
    }

    pub fn get_mut(&mut self, page_idx: IVec) -> Option<&mut T> {
        let pos = self.index.get_index_of(&TO_XY(page_idx))?;
        self.handles.get_mut(pos)
    }

    pub fn has(&self, page_idx: IVec) -> bool {
        self.index.contains(&TO_XY(page_idx))
    }

    pub fn set(&mut self, page_idx: IVec, value: T) {
        let key = TO_XY(page_idx);
        let (pos, _) = self.index.insert_full(key);
        if pos < self.handles.len() {
            self.handles[pos] = value;
        } else {
            self.handles.push(value);
        }
        self.count += 1;
    }

    /// Removes the page and returns what it held, or None.
    pub fn clear(&mut self, page_idx: IVec) -> Option<T> {
        let key = TO_XY(page_idx);
        let pos = self.index.swap_remove_full(&key)?.0;
        // The last position moved into the removed one's place.
        let held = self.handles.swap_remove(pos);
        self.count += 1;
        Some(held)
    }

    /// The page key at a position, as the unsigned IVec it was set with.
    pub fn key_at(&self, pos: u32) -> IVec {
        self.index[pos as usize]
    }

    pub fn keys(&self) -> impl Iterator<Item = IVec> + '_ {
        self.index.iter().copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = (IVec, &T)> + '_ {
        self.index.iter().copied().zip(self.handles.iter())
    }
}

#[cfg(test)]
#[path = "page_table.test.rs"]
mod test;
