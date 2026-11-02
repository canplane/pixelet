// pxl-spec/space/hierarchy/sparse_tree.rs

//! A 2^D-ary tree under one node, kept in two flat lanes: `first_child` holds a
//! slot's child group (`BRANCH`) or nothing, a leaf; `data` a leaf's value,
//! or a branch's children as its writer folds them. A child group is a node's 2^D
//! children, named by its first slot; the first child group is the root's. A write splits leaves only on
//! its path, frees what it replaces, joins a child group of equal leaves into one,
//! so one picture has one tree, and folds every branch left on its path
//! again. A freed child group is reused and no other moves, so a subtree comes and
//! goes without the others' child groups changing place. The child groups a write
//! changes are listed for a copy of the lanes to take just those
//! (`take_written`): its path's, its leaf's and what it allocates; a freed
//! child group is no reader's until it is allocated again.

use super::tree::{children, slot_of};
use pxl_engine::overwrite::BRANCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot<T> {
    Leaf(T),
    /// Its child group, and their fold.
    Branch(u32, T),
}

pub struct SparseTree<T: Copy + PartialEq, const D: usize> {
    first_child: Vec<u32>,
    data: Vec<T>,
    free: Vec<u32>,
    /// The first slots of the child groups written since they were last taken, a
    /// child group once a write.
    written_groups: Vec<u32>,
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
            first_child: vec![0; Self::WIDTH],
            data: vec![val; Self::WIDTH],
            free: Vec::new(),
            written_groups: vec![0],
        }
    }

    /// The child groups written since they were last taken, their first slots, into
    /// `into`, which held the last taken; none listed after.
    pub fn take_written(&mut self, into: &mut Vec<u32>) {
        into.clear();
        core::mem::swap(into, &mut self.written_groups);
    }

    /// Lists the child group `slot` lies in: child groups start a whole child group apart from
    /// the root's (`alloc`).
    fn touch(&mut self, slot: usize) {
        self.written_groups.push((slot - slot % Self::WIDTH) as u32);
    }

    /// The child pointer lane, a word a slot: its child group with `BRANCH`, or 0.
    pub fn first_child(&self) -> &[u32] {
        &self.first_child
    }

    /// Slots either lane has room for before it grows.
    pub fn room(&self) -> usize {
        self.first_child.capacity().min(self.data.capacity())
    }

    /// The data lane, a value a slot: a leaf's, or a branch's fold.
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// A branch's child `k`, in `slot_of`'s order.
    pub fn child(&self, child_group: u32, k: usize) -> Slot<T> {
        self.slot(child_group as usize + k)
    }

    /// The node `depth` levels under the root holding `at`, its index at that
    /// depth: its slot and how deep it is, or the leaf above it and its depth.
    pub fn get(&self, depth: u32, at: [i32; D]) -> (u32, Slot<T>) {
        xpute_kit::ensure!(depth >= 1, EINVAL, depth);
        let mut base = 0;
        for d in 1..=depth {
            match self.slot(base + toward(at, depth - d)) {
                Slot::Branch(child_group, _) if d < depth => base = child_group as usize,
                s => return (d, s),
            }
        }
        xpute_kit::bug!(EINVAL, depth)
    }

    /// Makes the node `depth` levels under the root holding `at` one leaf of
    /// `value`, each branch over it its child group `fold` folds, given its depth;
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
            if self.first_child[slot] == 0 {
                if self.data[slot] == val {
                    return false;
                }
                let child_group = self.alloc(self.data[slot]);
                self.first_child[slot] = child_group | BRANCH;
                self.touch(slot);
            }
            path[d as usize] = slot;
            base = (self.first_child[slot] & !BRANCH) as usize;
        }
        let slot = base + toward(at, 0);
        if self.first_child[slot] == 0 && self.data[slot] == val {
            return false;
        }
        self.release(slot);
        self.data[slot] = val;
        self.touch(slot);
        self.refold(&path, depth, fold);
        true
    }

    /// Makes the node `depth` levels under the root holding `at` a branch
    /// of `empty` leaves holding `val` as their fold: a node whose leaves
    /// are not here, folded where they are. Each branch over it its child
    /// group `fold` folds, given its depth. A write under it folds it from
    /// what is there.
    pub fn set_branch(
        &mut self,
        depth: u32,
        at: [i32; D],
        val: T,
        empty: T,
        fold: impl Fn(&Self, u32, u32) -> T,
    ) {
        xpute_kit::ensure!((1..=32).contains(&depth), EINVAL, depth);
        let mut path = [0usize; 32];
        let mut base = 0;
        for d in 1..depth {
            let slot = base + toward(at, depth - d);
            if self.first_child[slot] == 0 {
                let child_group = self.alloc(self.data[slot]);
                self.first_child[slot] = child_group | BRANCH;
                self.touch(slot);
            }
            path[d as usize] = slot;
            base = (self.first_child[slot] & !BRANCH) as usize;
        }
        let slot = base + toward(at, 0);
        self.release(slot);
        let child_group = self.alloc(empty);
        self.first_child[slot] = child_group | BRANCH;
        self.data[slot] = val;
        self.touch(slot);
        self.refold(&path, depth, fold);
    }

    /// Each branch on `path` over a node `depth` levels down, from the
    /// deepest: a child group of equal leaves joined into one, any other
    /// folded by `fold`.
    fn refold(&mut self, path: &[usize; 32], depth: u32, fold: impl Fn(&Self, u32, u32) -> T) {
        for d in (1..depth).rev() {
            let parent = path[d as usize];
            let child_group = (self.first_child[parent] & !BRANCH) as usize;
            if (child_group..child_group + Self::WIDTH)
                .all(|k| self.first_child[k] == 0 && self.data[k] == self.data[child_group])
            {
                self.data[parent] = self.data[child_group];
                self.release(parent);
            } else {
                self.data[parent] = fold(self, child_group as u32, d);
            }
            self.touch(parent);
        }
    }

    /// What slot `slot` holds.
    pub fn slot(&self, slot: usize) -> Slot<T> {
        match self.first_child[slot] {
            0 => Slot::Leaf(self.data[slot]),
            h => Slot::Branch(h & !BRANCH, self.data[slot]),
        }
    }

    /// A child group of leaves of `value`, a freed one first.
    fn alloc(&mut self, val: T) -> u32 {
        let child_group = match self.free.pop() {
            Some(b) => b as usize,
            None => {
                let b = self.first_child.len();
                self.first_child.resize(b + Self::WIDTH, 0);
                self.data.resize(b + Self::WIDTH, val);
                b
            }
        };
        self.first_child[child_group..child_group + Self::WIDTH].fill(0);
        self.data[child_group..child_group + Self::WIDTH].fill(val);
        self.touch(child_group);
        child_group as u32
    }

    /// Makes `slot` a leaf, freeing every child group under it.
    fn release(&mut self, slot: usize) {
        let h = core::mem::take(&mut self.first_child[slot]);
        if h == 0 {
            return;
        }
        let child_group = (h & !BRANCH) as usize;
        for k in 0..Self::WIDTH {
            self.release(child_group + k);
        }
        self.free.push(child_group as u32);
    }
}
