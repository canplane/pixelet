// pxl-kernel/layer/object/pyramid.rs

//! The objects' pyramid: what each node holds at every zoom, in the store's
//! one tree itself, as the bake folds the ground's tiles a zoom at a time. A
//! leaf placed is its value; a branch's slot holds its eight children folded
//! (`fold_cell`, the majority of their fill), which each write folds again
//! up its path. The objects are not fetched today, so the layer folds its
//! own pyramid from its writes, where a served tile would otherwise arrive.
//! A reader asks a node by one look-up, what covers it handed down from its
//! parent (pxl-portable `placed`), and never folds what is under it.

use pxl_spec::block::placed::{voxel, FULL};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::sparse_tree::Slot;

use super::store::{Leaf, Tree, EMPTY};

/// A branch's word in the tree: its fold, the kind most of its fill is and
/// the share filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fold {
    pub kind: u16,
    pub fill: u16,
}

impl Fold {
    pub(super) fn of(word: u32) -> Fold {
        Fold {
            kind: word as u16,
            fill: (word >> 16) as u16,
        }
    }

    fn word(self) -> u32 {
        u32::from(self.kind) | u32::from(self.fill) << 16
    }
}

/// A branch of `tree` at `zoom`, its block's eight folded, as its word.
pub(super) fn fold(tree: &Tree, block: u32, zoom: Zoom) -> u32 {
    fold_cell(core::array::from_fn(|k| share(tree.child(block, k))), zoom).word()
}

/// A child's share in its parent's fold: a placed voxel fills it whole.
fn share(slot: Slot<u32>) -> Fold {
    match slot {
        Slot::Leaf(v) => {
            let leaf = Leaf::of(v);
            match leaf != EMPTY && voxel(u32::from(leaf.id)) {
                true => Fold { kind: leaf.id, fill: FULL as u16 },
                false => Fold { kind: EMPTY.id, fill: 0 },
            }
        }
        Slot::Branch(_, f) => Fold::of(f),
    }
}

/// The 2×2×2 children folded (pxl-spec bake/fold.rs): the share of the cube
/// filled, the mean of theirs, and the kind most of that fill is, a tie to
/// the corner `majority`'s alternates to. A non-voxel value fills nothing.
fn fold_cell(children: [Fold; 8], zoom: Zoom) -> Fold {
    let fill = children.iter().map(|c| u32::from(c.fill)).sum::<u32>() / 8;
    let of = |kind: u16| children.iter().filter(|c| c.kind == kind).map(|c| u32::from(c.fill)).sum::<u32>();
    let mut order = children;
    if zoom & 1 == 0 {
        order.reverse();
    }
    let mut best = (EMPTY.id, 0);
    for c in order {
        let w = of(c.kind);
        if c.fill > 0 && w > best.1 {
            best = (c.kind, w);
        }
    }
    Fold { kind: best.0, fill: fill as u16 }
}
