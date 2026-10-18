// pxl-spec/block/volume.rs

//! The objects' sparse tree folded as it is written (pxl-kernel
//! layer/object/store.rs), each branch its eight by the overwrites' one
//! rule (pxl-portable `overwrite::fold`).

use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_portable::overwrite;

/// A branch of `tree` at `zoom`, its block's eight folded, as its word.
pub fn fold(tree: &SparseTree<u32, 3>, block: u32, zoom: Zoom) -> u32 {
    overwrite::fold(core::array::from_fn(|k| share(tree.child(block, k))), zoom)
}

/// A child's fold word in its parent's fold (pxl-portable
/// `overwrite::fold`): a leaf's share (`overwrite::share`), a branch's fold.
fn share(slot: Slot<u32>) -> u32 {
    match slot {
        Slot::Leaf(word) => overwrite::share(word),
        Slot::Branch(_, f) => f,
    }
}
