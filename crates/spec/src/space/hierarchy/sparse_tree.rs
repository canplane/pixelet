// pxl-spec/space/hierarchy/sparse_tree.rs

//! 2^D-ary trees under one or more roots, kept in two flat lanes: `heap`
//! holds a slot's child block (`BRANCH`) or nothing, a leaf; `data` a leaf's
//! value, or a branch's children as its writer folds them. A block is a
//! node's 2^D children, named by its first slot; the first blocks are the
//! roots', in order, so trees of one shape share the lanes and a reader of
//! them. A write splits leaves only on
//! its path, frees what it replaces, joins a block of equal leaves into one,
//! so one picture has one tree, and folds every branch left on its path
//! again. A freed block is reused and no other moves, so a subtree comes and
//! goes without the others' blocks changing place.

use super::tree::{children, slot_of};
use pxl_portable::placed::BRANCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot<T> {
    Leaf(T),
    /// Its children's block, and their fold.
    Branch(u32, T),
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

    /// `roots` roots, each one's children a leaf of `value`.
    pub fn new(value: T, roots: usize) -> SparseTree<T, D> {
        SparseTree {
            heap: vec![0; roots * Self::WIDTH],
            data: vec![value; roots * Self::WIDTH],
            free: Vec::new(),
        }
    }

    /// The heap lane, a word a slot: its child block with `BRANCH`, or 0.
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

    /// Root `root`'s block, its children's.
    pub const fn root(root: usize) -> u32 {
        (root * Self::WIDTH) as u32
    }

    /// A branch's child `k`, in `slot_of`'s order.
    pub fn child(&self, block: u32, k: usize) -> Slot<T> {
        self.slot(block as usize + k)
    }

    /// The node `depth` levels under root `root` holding `at`, its index at
    /// that depth: its slot and how deep it is, or the leaf above it and its
    /// depth.
    pub fn get(&self, root: usize, depth: u32, at: [i64; D]) -> (u32, Slot<T>) {
        xpute_kit::ensure!(depth >= 1, EINVAL, depth);
        let mut base = Self::root(root) as usize;
        for d in 1..=depth {
            match self.slot(base + toward(at, depth - d)) {
                Slot::Branch(block, _) if d < depth => base = block as usize,
                s => return (d, s),
            }
        }
        xpute_kit::bug!(EINVAL, depth)
    }

    /// Makes the node `depth` levels under root `root` holding `at` one leaf
    /// of `value`, each branch over it its block `fold` folds, given its
    /// depth; whether anything changed.
    pub fn set(&mut self, root: usize, depth: u32, at: [i64; D], value: T, fold: impl Fn(&Self, u32, u32) -> T) -> bool {
        xpute_kit::ensure!((1..=32).contains(&depth), EINVAL, depth);
        let mut path = [0usize; 32];
        let mut base = Self::root(root) as usize;
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
            if (block..block + Self::WIDTH).all(|k| self.heap[k] == 0 && self.data[k] == self.data[block]) {
                self.data[parent] = self.data[block];
                self.release(parent);
            } else {
                self.data[parent] = fold(self, block as u32, d);
            }
        }
        true
    }

    fn slot(&self, slot: usize) -> Slot<T> {
        match self.heap[slot] {
            0 => Slot::Leaf(self.data[slot]),
            h => Slot::Branch(h & !BRANCH, self.data[slot]),
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
