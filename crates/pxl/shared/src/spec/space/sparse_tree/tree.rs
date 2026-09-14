// pxl-shared/spec/space/sparse_tree/tree.rs
// sparse tree (2^d-ary sparse tree; quadtree/octree are special cases)

use xpute_core::ds::typed_alloc::TypedAlloc;
use xpute_core::kit::array::{NumberElement, TypedArray, U64Array};

use crate::spec::space::pvf3::{pvf3, IVec, Ivec, PVecTraits, PVF3};

use super::mem as st_mem;
pub use super::mem::SparseTreeLayout;
use super::mem::SparseTreeMemory;

pub struct SparseTree<T: NumberElement> {
    pub layout: SparseTreeLayout,
    pub mem: SparseTreeMemory<T>,
}

// ---- Lifecycle & Codec ----

/// Convert PVF3 to a sparse-tree leaf key at min resolution.
///
/// Semantics:
/// - sparse-tree traversal consumes the integer lattice key only
/// - PVF3 fractional residue is discarded by shifting down to the leaf scale
fn to_key(coord: &PVF3, min: i32) -> IVec {
    let mut reg = PVF3::default();
    pvf3::sh(&mut reg, coord, -min).i
}

/// Allocate a sparse tree with the given layout and initial capacity, its
/// lanes from `typed` (the heap unless given).
pub fn alloc<T: NumberElement>(layout: SparseTreeLayout, cap: u32, typed: Option<Box<dyn TypedAlloc>>) -> SparseTree<T> {
    SparseTree {
        layout,
        mem: st_mem::alloc(layout.dim, cap, typed),
    }
}

/// Hands the tree's lanes back. The tree must not be used afterwards.
pub fn free<T: NumberElement>(tree: &mut SparseTree<T>) {
    st_mem::free(&mut tree.mem);
}

/// Moves the tree's leaf lane to another type, every value carried across —
/// widening a lane whose values outgrew it. In place there; a new tree of
/// the other element here, the old one consumed.
pub fn retype<T: NumberElement, U: NumberElement>(tree: SparseTree<T>) -> SparseTree<U> {
    SparseTree {
        layout: tree.layout,
        mem: st_mem::retype(tree.mem),
    }
}

/// Encode the sparse tree backing memory into the wire/storage blob format.
pub fn encode<T: NumberElement>(tree: &SparseTree<T>) -> U64Array {
    st_mem::encode(&tree.mem)
}

/// U64Array -> SparseTree<T>. Decode a wire/storage blob into a sparse tree with the given layout.
pub fn decode<T: NumberElement>(buf: &U64Array, layout: SparseTreeLayout, typed: Option<Box<dyn TypedAlloc>>) -> SparseTree<T> {
    let mem = st_mem::decode::<T>(buf);
    SparseTree {
        layout,
        mem: if let Some(typed) = typed { st_mem::adopt(&mem, typed) } else { mem },
    }
}

/// SparseTree<T> -> T. Reconstruct the dense array from an already-decoded sparse tree.
pub fn dense<K: PVecTraits, T: NumberElement>(tree: &SparseTree<T>) -> TypedArray<T>
where
    K::V: Copy,
{
    st_mem::dense::<K, T>(&tree.mem, &tree.layout, 0)
}

// ---- I/O ----

/// Write one value at the given PVF3 coordinate. `level` (the finest,
/// `tree.layout.min`, for a leaf) claims a whole block at that coarser
/// shift amount in one call instead of always writing a single finest-leaf
/// cell — see st_mem::set's own doc comment.
pub fn write<T: NumberElement>(tree: &mut SparseTree<T>, coord: &PVF3, val: T, level: i32) {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::set::<Ivec, T>(&mut tree.mem, &tree.layout, key, val, level, 0);
}

/// Read one value at the given PVF3 coordinate.
pub fn read<T: NumberElement>(tree: &SparseTree<T>, coord: &PVF3) -> T {
    let key: IVec = to_key(coord, tree.layout.min);
    st_mem::get::<Ivec, T>(&tree.mem, &tree.layout, key, 0)
}

/// Reset the tree to a uniform root value.
pub fn fill<T: NumberElement>(tree: &mut SparseTree<T>, val: T) {
    st_mem::fill(&mut tree.mem, &tree.layout, val);
}

/// Compact the tree in-place by pruning and relocating live blocks.
pub fn compact<T: NumberElement>(tree: &mut SparseTree<T>) -> &mut SparseTree<T> {
    let compacted = st_mem::compact(&mut tree.mem, &tree.layout, 0).mem;
    let mut old = core::mem::replace(&mut tree.mem, compacted);
    st_mem::free(&mut old);
    tree
}

/// Visit every maximal uniform leaf block — see st_mem::walk's own doc
/// comment. `x`/`y`/`z` are in min-level lattice units (multiply by
/// `2**tree.layout.min` for a real-unit corner); `level` is that block's
/// own shift amount (`2**level` is its real-unit size).
pub fn walk<T: NumberElement>(tree: &SparseTree<T>, visit: &mut dyn FnMut(i32, i32, i32, i32, T)) {
    st_mem::walk(&tree.mem, &tree.layout, visit, 0);
}

/// Replace tree contents from a dense leaf array.
///
/// Dense layout contract:
/// - dim=2: row-major, x contiguous inside each row
/// - dim=3: slab-major [z][y][x], x contiguous inside each row
///
/// This is the preferred path when the producer already rasterized into a dense buffer.
pub fn build_dense<'a, T: NumberElement>(tree: &'a mut SparseTree<T>, leaves: &[T]) -> &'a mut SparseTree<T> {
    tree.mem = st_mem::alloc(tree.layout.dim, leaves.len() as u32, None);

    let root = st_mem::build(&mut tree.mem, &tree.layout, leaves);
    tree.mem = st_mem::compact(&mut tree.mem, &tree.layout, root).mem;
    tree
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod test;
