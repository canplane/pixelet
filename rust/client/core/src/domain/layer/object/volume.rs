// pxl-core/domain/layer/object/volume.rs

//! The objects' sparse tree, folded as it is written (store.rs), each branch holding its eight
//! children folded by the overwrites' one rule (pxl-engine `object::fold`).

use crate::domain::layer::object::sparse_tree::{Slot, SparseTree};
use pxl_engine::object;
use pxl_spec::space::coord::zoom::Zoom;

/// A branch of `tree` at `zoom`, its child group's eight folded, as its word.
pub fn fold(tree: &SparseTree<u32, 3>, child_group: u32, zoom: Zoom) -> u32 {
    object::fold(
        core::array::from_fn(|k| share(tree.child(child_group, k))),
        zoom,
    )
}

/// A child's fold word in its parent's fold (pxl-engine
/// `object::fold`): a leaf's share (`object::share`), a branch's fold.
fn share(slot: Slot<u32>) -> u32 {
    match slot {
        Slot::Leaf(word) => object::share(word),
        Slot::Branch(_, f) => f,
    }
}
