// pxl-spec/space/sparse_tree/tree.rs
// sparse tree (2^d-ary sparse tree; quadtree/octree are special cases)

use crate::space::hierarchy::Scale;
use crate::space::pvf3::{pvf3, IVec, Ivec, PVecTraits, Pvf3};

use super::mem as st_mem;
pub use super::mem::SparseTreeLayout;
use super::mem::SparseTreeMemory;

pub struct SparseTree<T: Copy + Default + PartialEq> {
    pub layout: SparseTreeLayout,
    pub mem: SparseTreeMemory<T>,
}

// ---- Lifecycle & Codec ----

/// Convert PVF3 to a sparse-tree leaf key at min resolution.
///
/// Semantics:
/// - sparse-tree traversal consumes the integer lattice key only
/// - PVF3 fractional residue is discarded by shifting down to the leaf scale
fn to_key(coord: &Pvf3, min: Scale) -> IVec {
    let mut reg = Pvf3::default();
    pvf3::sh(&mut reg, coord, -min.shamt()).i
}

/// Allocate a sparse tree with the given layout and initial capacity.
pub fn alloc<T: Copy + Default + PartialEq>(layout: SparseTreeLayout, cap: u32) -> SparseTree<T> {
    SparseTree {
        layout,
        mem: st_mem::alloc(layout.dim, cap),
    }
}

/// Moves the tree's leaf lane to another type, every value carried across —
/// widening a lane whose values outgrew it. In place there; a new tree of
/// the other element here, the old one consumed.
pub fn retype<T: Copy + Default + PartialEq, U: Copy + Default + PartialEq + From<T>>(tree: SparseTree<T>) -> SparseTree<U> {
    SparseTree {
        layout: tree.layout,
        mem: st_mem::retype(tree.mem),
    }
}

/// Encode the sparse tree backing memory into the wire/storage blob format.
pub fn encode<T: Copy + Default + PartialEq>(tree: &SparseTree<T>) -> Vec<u64> {
    st_mem::encode(&tree.mem)
}

/// &[u64] -> SparseTree<T>. Decode a wire/storage blob into a sparse tree with the given layout.
pub fn decode<T: Copy + Default + PartialEq>(buf: &[u64], layout: SparseTreeLayout) -> SparseTree<T> {
    SparseTree {
        layout,
        mem: st_mem::decode::<T>(buf),
    }
}

/// SparseTree<T> -> T. Reconstruct the dense array from an already-decoded sparse tree.
pub fn dense<K: PVecTraits, T: Copy + Default + PartialEq>(tree: &SparseTree<T>) -> Box<[T]>
where
    K::V: Copy,
{
    st_mem::dense::<K, T>(&tree.mem, &tree.layout, 0)
}

// ---- I/O ----

/// Write one value at the given PVF3 coordinate. `level` (the finest,
/// `tree.layout.min`, for a leaf) claims a whole block at that coarser
/// scale in one call instead of always writing a single finest-leaf
/// cell — see st_mem::set's own doc comment.
pub fn write<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>, coord: &Pvf3, val: T, level: Scale) {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::set::<Ivec, T>(&mut tree.mem, &tree.layout, key, val, level, 0);
}

/// Read one value at the given PVF3 coordinate.
pub fn read<T: Copy + Default + PartialEq>(tree: &SparseTree<T>, coord: &Pvf3) -> T {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::get::<Ivec, T>(&tree.mem, &tree.layout, key, 0)
}

/// Reset the tree to a uniform root value.
pub fn fill<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>, val: T) {
    st_mem::fill(&mut tree.mem, &tree.layout, val);
}

/// Compact the tree in-place by pruning and relocating live blocks.
pub fn compact<T: Copy + Default + PartialEq>(tree: &mut SparseTree<T>) -> &mut SparseTree<T> {
    tree.mem = st_mem::compact(&mut tree.mem, &tree.layout, 0).mem;
    tree
}

/// Visit every maximal uniform leaf block — see st_mem::walk's own doc
/// comment. `x`/`y`/`z` are in min-level lattice units (multiply by
/// `tree.layout.min.side()` for a real-unit corner); `level` is that block's
/// own scale.
pub fn walk<T: Copy + Default + PartialEq>(tree: &SparseTree<T>, visit: &mut dyn FnMut(i32, i32, i32, Scale, T)) {
    st_mem::walk(&tree.mem, &tree.layout, visit, 0);
}

/// Replace tree contents from a dense leaf array.
///
/// Dense layout contract:
/// - dim=2: row-major, x contiguous inside each row
/// - dim=3: slab-major [z][y][x], x contiguous inside each row
///
/// This is the preferred path when the producer already rasterized into a dense buffer.
pub fn build_dense<'a, T: Copy + Default + PartialEq>(tree: &'a mut SparseTree<T>, leaves: &[T]) -> &'a mut SparseTree<T> {
    tree.mem = st_mem::alloc(tree.layout.dim, leaves.len() as u32);

    let root = st_mem::build(&mut tree.mem, &tree.layout, leaves);
    tree.mem = st_mem::compact(&mut tree.mem, &tree.layout, root).mem;
    tree
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod test;
