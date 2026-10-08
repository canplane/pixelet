// pxl-spec/space/hierarchy/sparse_tree.rs

//! A 2^D-ary tree under one node, kept in two flat lanes: `heap` holds a
//! slot's child block (`BRANCH`) or nothing, a leaf; `data` a leaf's value.
//! A block is a node's 2^D children; the first block is the root's. Only
//! leaves carry values: nothing is kept of what a branch holds. A write
//! splits leaves only on its path, frees what it replaces, and joins a block
//! of equal leaves into one, so one picture has one tree; a freed block is
//! reused.

use super::tree::{children, slot_of};

const BRANCH: u32 = 1 << 31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot<T> {
    Leaf(T),
    /// Its children's block.
    Branch(u32),
}

pub struct SparseTree<T: Copy + PartialEq, const D: usize> {
    heap: Vec<u32>,
    data: Vec<T>,
    free: Vec<u32>,
}

/// The child on the way to `at`, `steps` levels over it.
fn toward<const D: usize>(at: [i64; D], steps: u32) -> usize {
    slot_of(at.map(|v| v >> steps))
}

impl<T: Copy + PartialEq, const D: usize> SparseTree<T, D> {
    const WIDTH: usize = children(D);

    /// The root's children, each a leaf of `value`.
    pub fn new(value: T) -> SparseTree<T, D> {
        SparseTree {
            heap: vec![0; Self::WIDTH],
            data: vec![value; Self::WIDTH],
            free: Vec::new(),
        }
    }

    /// The one value the whole tree is, if it is one.
    pub fn uniform(&self) -> Option<T> {
        (0..Self::WIDTH).all(|s| self.heap[s] == 0 && self.data[s] == self.data[0]).then_some(self.data[0])
    }

    /// A branch's children in `slot_of`'s order, the root's block at 0.
    pub fn block(&self, block: u32) -> impl Iterator<Item = Slot<T>> + '_ {
        (0..Self::WIDTH).map(move |k| self.slot(block as usize + k))
    }

    /// The node `depth` levels under the root holding `at`, its index at that
    /// depth: its slot and how deep it is, or the leaf above it and its depth.
    pub fn get(&self, depth: u32, at: [i64; D]) -> (u32, Slot<T>) {
        xpute_kit::ensure!(depth >= 1, EINVAL, depth);
        let mut base = 0;
        for d in 1..=depth {
            match self.slot(base + toward(at, depth - d)) {
                Slot::Branch(block) if d < depth => base = block as usize,
                s => return (d, s),
            }
        }
        xpute_kit::bug!(EINVAL, depth)
    }

    /// Makes the node `depth` levels under the root holding `at` one leaf of
    /// `value`; whether anything changed.
    pub fn set(&mut self, depth: u32, at: [i64; D], value: T) -> bool {
        xpute_kit::ensure!((1..=32).contains(&depth), EINVAL, depth);
        let mut path = [0usize; 32];
        let mut base = 0;
        for d in 1..depth {
            let slot = base + toward(at, depth - d);
            if self.heap[slot] == 0 {
                if self.data[slot] == value {
                    return false;
                }
                let block = self.alloc(self.data[slot]);
                self.heap[slot] = block | BRANCH;
            }
            path[d as usize] = slot;
            base = (self.heap[slot] & !BRANCH) as usize;
        }
        let slot = base + toward(at, 0);
        if self.heap[slot] == 0 && self.data[slot] == value {
            return false;
        }
        self.release(slot);
        self.data[slot] = value;
        for d in (1..depth).rev() {
            let parent = path[d as usize];
            let block = (self.heap[parent] & !BRANCH) as usize;
            if !(block..block + Self::WIDTH).all(|k| self.heap[k] == 0 && self.data[k] == self.data[block]) {
                break;
            }
            self.data[parent] = self.data[block];
            self.release(parent);
        }
        true
    }

    fn slot(&self, slot: usize) -> Slot<T> {
        match self.heap[slot] {
            0 => Slot::Leaf(self.data[slot]),
            h => Slot::Branch(h & !BRANCH),
        }
    }

    /// A block of leaves of `value`, a freed one first.
    fn alloc(&mut self, value: T) -> u32 {
        let block = match self.free.pop() {
            Some(b) => b as usize,
            None => {
                let b = self.heap.len();
                self.heap.resize(b + Self::WIDTH, 0);
                self.data.resize(b + Self::WIDTH, value);
                b
            }
        };
        self.heap[block..block + Self::WIDTH].fill(0);
        self.data[block..block + Self::WIDTH].fill(value);
        block as u32
    }

    /// Makes `slot` a leaf, freeing every block under it.
    fn release(&mut self, slot: usize) {
        let h = core::mem::take(&mut self.heap[slot]);
        if h == 0 {
            return;
        }
        let block = (h & !BRANCH) as usize;
        for k in 0..Self::WIDTH {
            self.release(block + k);
        }
        self.free.push(block as u32);
    }
}
