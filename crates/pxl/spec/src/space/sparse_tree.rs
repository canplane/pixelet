// pxl-spec/space/sparse_tree.rs

//! An octree kept in two flat lanes: `heap` holds a slot's child block
//! (`BRANCH` set) or nothing, a leaf; `data` holds a leaf's value or what a
//! branch's children fold to. A block is eight slots; the first eight are
//! the root's, the cells of zoom 1. A node is named as `wrap_index` names
//! an X, signed about zero on every axis, so the tree is the world round in X
//! and past what Y and Z ever reach. A write changes the path to its node and
//! nothing beside it: the path is refolded, a uniform block collapses into
//! its parent, and a freed block is reused.

use super::hierarchy::{child_of, children, slot_of};
use super::zoom::Zoom;

const BRANCH: u32 = 1 << 31;

/// A node as the tree holds it: a leaf's value, or a branch's fold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node<T> {
    pub zoom: Zoom,
    pub at: [i64; 3],
    pub data: T,
    /// A branch's child block.
    block: Option<u32>,
}

impl<T> Node<T> {
    pub fn branch(&self) -> bool {
        self.block.is_some()
    }
}

pub struct SparseTree<T: Copy + Default + PartialEq> {
    heap: Vec<u32>,
    data: Vec<T>,
    free: Vec<u32>,
}

const WIDTH: usize = children(3);

/// The root block's cells: one bit an axis at zoom 1, signed as `wrap_index` names it.
fn top(slot: usize) -> [i64; 3] {
    child_of([0; 3], slot).map(|b| -b)
}

/// The child of the zoom-`zoom - steps` node on the way to `at`.
fn slot_toward(at: [i64; 3], steps: u32) -> usize {
    slot_of(at.map(|v| v >> steps))
}

impl<T: Copy + Default + PartialEq> Default for SparseTree<T> {
    fn default() -> SparseTree<T> {
        SparseTree {
            heap: vec![0; WIDTH],
            data: vec![T::default(); WIDTH],
            free: Vec::new(),
        }
    }
}

impl<T: Copy + Default + PartialEq> SparseTree<T> {
    pub fn is_empty(&self) -> bool {
        (0..WIDTH).all(|s| self.heap[s] == 0 && self.data[s] == T::default())
    }

    /// The node at `zoom` holding `at`, or the leaf above it where one stops the way.
    pub fn get(&self, zoom: Zoom, at: [i64; 3]) -> Node<T> {
        let mut base = 0;
        for z in 1..=zoom {
            let node = self.node(base + slot_toward(at, u32::from(zoom - z)), z, at.map(|v| v >> (zoom - z)));
            match node.block {
                Some(block) if z < zoom => base = block as usize,
                _ => return node,
            }
        }
        xpute_core::bug!(EINVAL, zoom)
    }

    /// Makes the node at `zoom` holding `at` one leaf of `value`; false where it
    /// already was. `fold` makes a branch's data from its children's, and
    /// whether each is a branch, at the branch's zoom.
    pub fn set(&mut self, zoom: Zoom, at: [i64; 3], value: T, fold: impl Fn(&[T], &[bool], Zoom) -> T) -> bool {
        xpute_core::ensure!(zoom >= 1, EINVAL, zoom);
        let mut path = Vec::with_capacity(usize::from(zoom));
        let mut base = 0;
        for z in 1..zoom {
            let slot = base + slot_toward(at, u32::from(zoom - z));
            if self.heap[slot] & BRANCH == 0 {
                if self.data[slot] == value {
                    return false;
                }
                // A leaf the write lands under splits into its eight.
                let block = self.alloc(self.data[slot]);
                self.heap[slot] = block as u32 | BRANCH;
            }
            path.push((slot, z));
            base = (self.heap[slot] & !BRANCH) as usize;
        }
        let slot = base + slot_toward(at, 0);
        if self.heap[slot] & BRANCH == 0 && self.data[slot] == value {
            return false;
        }
        self.release(slot);
        self.data[slot] = value;
        for &(slot, z) in path.iter().rev() {
            let block = (self.heap[slot] & !BRANCH) as usize;
            let kids = block..block + WIDTH;
            let uniform = kids.clone().all(|k| self.heap[k] == 0 && self.data[k] == self.data[block]);
            if uniform {
                self.data[slot] = self.data[block];
                self.release(slot);
                continue;
            }
            let branch: [bool; WIDTH] = core::array::from_fn(|k| self.heap[block + k] != 0);
            self.data[slot] = fold(&self.data[kids], &branch, z);
        }
        true
    }

    /// The root block's nodes, the cells of zoom 1.
    pub fn roots(&self) -> [Node<T>; WIDTH] {
        core::array::from_fn(|slot| self.node(slot, 1, top(slot)))
    }

    pub fn children(&self, n: &Node<T>) -> Option<[Node<T>; WIDTH]> {
        let block = n.block? as usize;
        Some(core::array::from_fn(|k| self.node(block + k, n.zoom + 1, child_of(n.at, k))))
    }

    /// Every node from `from` down, a branch entered where `enter` says so.
    pub fn visit(&self, from: &[Node<T>], enter: &mut dyn FnMut(&Node<T>) -> bool) {
        for n in from {
            if enter(n) {
                if let Some(kids) = self.children(n) {
                    self.visit(&kids, enter);
                }
            }
        }
    }

    fn node(&self, slot: usize, zoom: Zoom, at: [i64; 3]) -> Node<T> {
        let h = self.heap[slot];
        Node {
            zoom,
            at,
            data: self.data[slot],
            block: (h & BRANCH != 0).then_some(h & !BRANCH),
        }
    }

    /// A block of eight leaves of `value`, a freed one first.
    fn alloc(&mut self, value: T) -> usize {
        let block = match self.free.pop() {
            Some(b) => b as usize,
            None => {
                let b = self.heap.len();
                self.heap.resize(b + WIDTH, 0);
                self.data.resize(b + WIDTH, T::default());
                b
            }
        };
        self.heap[block..block + WIDTH].fill(0);
        self.data[block..block + WIDTH].fill(value);
        block
    }

    /// Makes `slot` a leaf, freeing every block under it.
    fn release(&mut self, slot: usize) {
        let node = core::mem::take(&mut self.heap[slot]);
        if node & BRANCH == 0 {
            return;
        }
        let block = (node & !BRANCH) as usize;
        for k in 0..WIDTH {
            self.release(block + k);
        }
        self.free.push(block as u32);
    }
}

#[cfg(test)]
#[path = "sparse_tree.test.rs"]
mod test;
