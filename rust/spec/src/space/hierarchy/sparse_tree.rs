// pxl-spec/space/hierarchy/sparse_tree.rs

//! A 2^D-ary tree under one node, kept in two flat lanes: `heap` holds a
//! slot's child family (`BRANCH`) or nothing, a leaf; `data` a leaf's value,
//! or a branch's children as its writer folds them. A family is a node's 2^D
//! children, named by its first slot; the first family is the root's. A write splits leaves only on
//! its path, frees what it replaces, joins a family of equal leaves into one,
//! so one picture has one tree, and folds every branch left on its path
//! again. A freed family is reused and no other moves, so a subtree comes and
//! goes without the others' families changing place. The families a write
//! changes are listed for a copy of the lanes to take just those
//! (`take_written`): its path's, its leaf's and what it allocates; a freed
//! family is no reader's until it is allocated again.

use super::tree::{children, slot_of};
use pxl_spec_core::overwrite::BRANCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot<T> {
    Leaf(T),
    /// Its children's family, and their fold.
    Branch(u32, T),
}

pub struct SparseTree<T: Copy + PartialEq, const D: usize> {
    heap: Vec<u32>,
    data: Vec<T>,
    free: Vec<u32>,
    /// The first slots of the families written since they were last taken, a
    /// family once a write.
    written: Vec<u32>,
}

/// The child on the way to `at`, `steps` levels over it.
fn toward<const D: usize>(at: [i32; D], steps: u32) -> usize {
    slot_of(at.map(|v| v >> steps))
}

impl<T: Copy + PartialEq, const D: usize> SparseTree<T, D> {
    const WIDTH: usize = children(D);

    /// The root's children, each a leaf of `value`.
    pub fn new(val: T) -> SparseTree<T, D> {
        SparseTree {
            heap: vec![0; Self::WIDTH],
            data: vec![val; Self::WIDTH],
            free: Vec::new(),
            written: vec![0],
        }
    }

    /// The families written since they were last taken, their first slots, into
    /// `into`, which held the last taken; none listed after.
    pub fn take_written(&mut self, into: &mut Vec<u32>) {
        into.clear();
        core::mem::swap(into, &mut self.written);
    }

    /// Lists the family `slot` lies in: families start a whole family apart from
    /// the root's (`alloc`).
    fn touch(&mut self, slot: usize) {
        self.written.push((slot - slot % Self::WIDTH) as u32);
    }

    /// The heap lane, a word a slot: its child family with `BRANCH`, or 0.
    pub fn heap(&self) -> &[u32] {
        &self.heap
    }

    /// Slots either lane has room for before it grows.
    pub fn room(&self) -> usize {
        self.heap.capacity().min(self.data.capacity())
    }

    /// The data lane, a value a slot: a leaf's, or a branch's fold.
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// A branch's child `k`, in `slot_of`'s order.
    pub fn child(&self, family: u32, k: usize) -> Slot<T> {
        self.slot(family as usize + k)
    }

    /// The node `depth` levels under the root holding `at`, its index at that
    /// depth: its slot and how deep it is, or the leaf above it and its depth.
    pub fn get(&self, depth: u32, at: [i32; D]) -> (u32, Slot<T>) {
        xpute_kit::ensure!(depth >= 1, EINVAL, depth);
        let mut base = 0;
        for d in 1..=depth {
            match self.slot(base + toward(at, depth - d)) {
                Slot::Branch(family, _) if d < depth => base = family as usize,
                s => return (d, s),
            }
        }
        xpute_kit::bug!(EINVAL, depth)
    }

    /// Makes the node `depth` levels under the root holding `at` one leaf of
    /// `value`, each branch over it its family `fold` folds, given its depth;
    /// whether anything changed.
    pub fn set(
        &mut self,
        depth: u32,
        at: [i32; D],
        val: T,
        fold: impl Fn(&Self, u32, u32) -> T,
    ) -> bool {
        xpute_kit::ensure!((1..=32).contains(&depth), EINVAL, depth);
        let mut path = [0usize; 32];
        let mut base = 0;
        for d in 1..depth {
            let slot = base + toward(at, depth - d);
            if self.heap[slot] == 0 {
                if self.data[slot] == val {
                    return false;
                }
                let family = self.alloc(self.data[slot]);
                self.heap[slot] = family | BRANCH;
                self.touch(slot);
            }
            path[d as usize] = slot;
            base = (self.heap[slot] & !BRANCH) as usize;
        }
        let slot = base + toward(at, 0);
        if self.heap[slot] == 0 && self.data[slot] == val {
            return false;
        }
        self.release(slot);
        self.data[slot] = val;
        self.touch(slot);
        for d in (1..depth).rev() {
            let parent = path[d as usize];
            let family = (self.heap[parent] & !BRANCH) as usize;
            if (family..family + Self::WIDTH)
                .all(|k| self.heap[k] == 0 && self.data[k] == self.data[family])
            {
                self.data[parent] = self.data[family];
                self.release(parent);
            } else {
                self.data[parent] = fold(self, family as u32, d);
            }
            self.touch(parent);
        }
        true
    }

    /// What slot `slot` holds.
    pub fn slot(&self, slot: usize) -> Slot<T> {
        match self.heap[slot] {
            0 => Slot::Leaf(self.data[slot]),
            h => Slot::Branch(h & !BRANCH, self.data[slot]),
        }
    }

    /// A family of leaves of `value`, a freed one first.
    fn alloc(&mut self, val: T) -> u32 {
        let family = match self.free.pop() {
            Some(b) => b as usize,
            None => {
                let b = self.heap.len();
                self.heap.resize(b + Self::WIDTH, 0);
                self.data.resize(b + Self::WIDTH, val);
                b
            }
        };
        self.heap[family..family + Self::WIDTH].fill(0);
        self.data[family..family + Self::WIDTH].fill(val);
        self.touch(family);
        family as u32
    }

    /// Makes `slot` a leaf, freeing every family under it.
    fn release(&mut self, slot: usize) {
        let h = core::mem::take(&mut self.heap[slot]);
        if h == 0 {
            return;
        }
        let family = (h & !BRANCH) as usize;
        for k in 0..Self::WIDTH {
            self.release(family + k);
        }
        self.free.push(family as u32);
    }
}
