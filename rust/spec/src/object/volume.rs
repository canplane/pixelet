// pxl-spec/object/volume.rs

//! The objects' sparse tree folded as it is written (pxl-kernel
//! domain/layer/object/store.rs), each branch its eight by the overwrites' one
//! rule (pxl-engine `overwrite::fold`).

use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::sparse_tree::{Slot, SparseTree};
use pxl_engine::overwrite;

/// A branch of `tree` at `zoom`, its child group's eight folded, as its word.
pub fn fold(tree: &SparseTree<u32, 3>, child_group: u32, zoom: Zoom) -> u32 {
    overwrite::fold(
        core::array::from_fn(|k| share(tree.child(child_group, k))),
        zoom,
    )
}

/// A child's fold word in its parent's fold (pxl-engine
/// `overwrite::fold`): a leaf's share (`overwrite::share`), a branch's fold.
fn share(slot: Slot<u32>) -> u32 {
    match slot {
        Slot::Leaf(word) => overwrite::share(word),
        Slot::Branch(_, f) => f,
    }
}
