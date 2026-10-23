// pxl-spec/block/volume.rs

//! A cube's eight children folded by volume, the one rule for what a node of
//! an overwrite's tree holds at a coarser zoom: the share of the cube filled,
//! the mean of theirs, and the kind most of that fill is. The objects'
//! store folds its tree by it as it is written (pxl-kernel
//! layer/object/store.rs), and the bake its tiles' ways.

use crate::block::placed::{voxel, FULL};
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::sparse_tree::{Slot, SparseTree};

/// A branch's word in a tree: its fold, the kind most of its fill is and the
/// share filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fold {
    pub kind: u16,
    pub fill: u16,
}

impl Fold {
    pub fn of(word: u32) -> Fold {
        Fold {
            kind: word as u16,
            fill: (word >> 16) as u16,
        }
    }

    pub fn word(self) -> u32 {
        u32::from(self.kind) | u32::from(self.fill) << 16
    }
}

/// A word no leaf holds: nothing there.
const EMPTY: u32 = 0;

/// A branch of `tree` at `zoom`, its block's eight folded, as its word.
pub fn fold(tree: &SparseTree<u32, 3>, block: u32, zoom: Zoom) -> u32 {
    fold_cell(core::array::from_fn(|k| share(tree.child(block, k))), zoom).word()
}

/// A child's share in its parent's fold: a leaf whose value is a voxel fills
/// it whole.
fn share(slot: Slot<u32>) -> Fold {
    match slot {
        Slot::Leaf(word) => {
            let id = word as u16;
            match word != EMPTY && voxel(u32::from(id)) {
                true => Fold { kind: id, fill: FULL as u16 },
                false => Fold::default(),
            }
        }
        Slot::Branch(_, f) => Fold::of(f),
    }
}

/// The 2×2×2 children folded (bake/fold.rs): the share of the cube filled,
/// the mean of theirs, and the kind most of that fill is, a tie to the
/// corner `majority`'s alternates to. A non-voxel value fills nothing.
fn fold_cell(children: [Fold; 8], zoom: Zoom) -> Fold {
    let fill = children.iter().map(|c| u32::from(c.fill)).sum::<u32>() / 8;
    let of = |kind: u16| children.iter().filter(|c| c.kind == kind).map(|c| u32::from(c.fill)).sum::<u32>();
    let mut order = children;
    if zoom & 1 == 0 {
        order.reverse();
    }
    let mut best = (Fold::default().kind, 0);
    for c in order {
        let w = of(c.kind);
        if c.fill > 0 && w > best.1 {
            best = (c.kind, w);
        }
    }
    Fold { kind: best.0, fill: fill as u16 }
}
