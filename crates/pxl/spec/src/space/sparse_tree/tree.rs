// pxl-spec/space/sparse_tree/tree.rs

use crate::space::hierarchy::Scale;
use crate::space::pvf3::{pvf3, IVec, Ivec, PVecTraits, Pvf3};

use super::mem as st_mem;
pub use super::mem::SparseTreeLayout;
use super::mem::SparseTreeMemory;

pub struct SparseTree<T: Copy + Default + PartialEq> {
    pub layout: SparseTreeLayout,
    pub mem: SparseTreeMemory<T>,
}

/// The leaf key at min resolution; the fraction is dropped.
fn to_key(coord: &Pvf3, min: Scale) -> IVec {
    let mut reg = Pvf3::default();
    pvf3::sh(&mut reg, coord, -min.shamt()).i
}

pub fn alloc<T: Copy + Default + PartialEq>(layout: SparseTreeLayout, cap: u32) -> SparseTree<T> {
    SparseTree {
        layout,
        mem: st_mem::alloc(layout.dim, cap),
    }
}

/// Widens the leaf lane to another type, consuming the tree.
pub fn retype<T: Copy + Default + PartialEq, U: Copy + Default + PartialEq + From<T>>(tree: SparseTree<T>) -> SparseTree<U> {
    SparseTree {
        layout: tree.layout,
        mem: st_mem::retype(tree.mem),
    }
}

pub fn encode<T: Copy + Default + PartialEq>(tree: &SparseTree<T>) -> Vec<u64> {
    st_mem::encode(&tree.mem)
}

pub fn decode<T: Copy + Default + PartialEq>(buf: &[u64], layout: SparseTreeLayout) -> SparseTree<T> {
    SparseTree {
        layout,
        mem: st_mem::decode::<T>(buf),
    }
}

/// The dense array (`st_mem::dense`'s layout).
pub fn dense<K: PVecTraits, T: Copy + Default + PartialEq>(tree: &SparseTree<T>) -> Box<[T]>
where
    K::V: Copy,
{
    st_mem::dense::<K, T>(&tree.mem, &tree.layout, 0)
}

/// `level` above `tree.layout.min` claims a whole block at once.
pub fn write<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>, coord: &Pvf3, val: T, level: Scale) {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::set::<Ivec, T>(&mut tree.mem, &tree.layout, key, val, level, 0);
}

pub fn read<T: Copy + Default + PartialEq>(tree: &SparseTree<T>, coord: &Pvf3) -> T {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::get::<Ivec, T>(&tree.mem, &tree.layout, key, 0)
}

pub fn fill<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>, val: T) {
    st_mem::fill(&mut tree.mem, &tree.layout, val);
}

pub fn compact<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>) -> &mut SparseTree<T> {
    tree.mem = st_mem::compact(&mut tree.mem, &tree.layout, 0).mem;
    tree
}

/// Every maximal uniform leaf block: corner in min-level units and the block's own scale.
pub fn walk<T: Copy + Default + PartialEq>(tree: &SparseTree<T>, visit: &mut dyn FnMut(i32, i32, i32, Scale, T)) {
    st_mem::walk(&tree.mem, &tree.layout, visit, 0);
}

/// Replace contents from a dense array: x fastest, then y, then z.
pub fn build_dense<'a, T: Copy + Default + PartialEq>(tree: &'a mut SparseTree<T>, leaves: &[T]) -> &'a mut SparseTree<T> {
    tree.mem = st_mem::alloc(tree.layout.dim, leaves.len() as u32);

    let root = st_mem::build(&mut tree.mem, &tree.layout, leaves);
    tree.mem = st_mem::compact(&mut tree.mem, &tree.layout, root).mem;
    tree
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod test;
