// pxl-spec/block/volume.rs

//! The objects' sparse tree folded as it is written (pxl-kernel
//! layer/object/store.rs), each branch its eight by the overwrites' one
//! rule (pxl-portable `overwrite::fold`).

use crate::block::overwrite::{self, voxel, FULL};
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::sparse_tree::{Slot, SparseTree};

/// A word no leaf holds: nothing there.
const EMPTY: u32 = 0;

/// A branch of `tree` at `zoom`, its block's eight folded, as its word.
pub fn fold(tree: &SparseTree<u32, 3>, block: u32, zoom: Zoom) -> u32 {
    overwrite::fold(core::array::from_fn(|k| share(tree.child(block, k))), zoom)
}

/// A child's fold word in its parent's fold (pxl-portable
/// `overwrite::fold`): a leaf whose value is a voxel fills it whole.
fn share(slot: Slot<u32>) -> u32 {
    match slot {
        Slot::Leaf(word) => {
            let id = overwrite::low(word);
            if word != EMPTY && voxel(id) {
                overwrite::fold_word(id, FULL)
            } else {
                0
            }
        }
        Slot::Branch(_, f) => f,
    }
}
