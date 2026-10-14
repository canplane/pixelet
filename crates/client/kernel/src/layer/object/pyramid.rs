// pxl-kernel/layer/object/pyramid.rs

//! The objects' pyramid: what each node holds at every zoom, in the store's
//! one tree itself, as the bake folds the ground's tiles a zoom at a time. A
//! leaf placed is its value; a branch's slot holds its eight children folded
//! (`fold_cell`, the majority of their fill), which each write folds again
//! up its path. The objects are not fetched today, so the layer folds its
//! own pyramid from its writes, where a served tile would otherwise arrive.
//! A reader asks a node by one look-up, what covers it handed down from its
//! parent (`Held`), and never folds what is under it.

use pxl_spec::block::object::object_value_is_voxel;
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::sparse_tree::Slot;
use pxl_spec::space::hierarchy::tree::slot_of;

use super::store::{EntryAt, Leaf, Tree, EMPTY};

/// A cube filled whole, as a fold's share counts it: a share is in 2⁻¹⁵ of
/// its cube, floored at each fold, so eighths stay exact five folds deep.
pub(super) const FULL: u16 = 1 << 15;

/// A branch's word in the tree: its fold, the kind most of its fill is and
/// the share filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fold {
    pub kind: u16,
    pub fill: u16,
}

impl Fold {
    fn of(word: u32) -> Fold {
        Fold {
            kind: word as u16,
            fill: (word >> 16) as u16,
        }
    }

    fn word(self) -> u32 {
        u32::from(self.kind) | u32::from(self.fill) << 16
    }
}

/// What a node holds, as a descent hands it down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    /// Nothing is placed under it.
    Nothing,
    /// A leaf placed at `zoom`, `at` covers it whole.
    Leaf { leaf: Leaf, zoom: Zoom, at: [i64; 3] },
    /// Leaves under it differ: their fold, and the block of its children.
    Folded { fold: Fold, block: u32 },
}

impl Held {
    /// What the node `at` of `zoom` holding `slot` holds, a leaf found over it
    /// at `found`.
    pub(super) fn of(slot: Slot<u32>, found: Zoom, (zoom, at): (Zoom, [i64; 3])) -> Held {
        match slot {
            Slot::Leaf(v) if v == EMPTY.word() => Held::Nothing,
            Slot::Leaf(v) => Held::Leaf {
                leaf: Leaf::of(v),
                zoom: found,
                at: wrap_cell(found, at.map(|v| v >> (zoom - found))),
            },
            Slot::Branch(block, f) => Held::Folded { fold: Fold::of(f), block },
        }
    }

    /// What the child at `zoom`, `at` of a node holding this holds: a leaf
    /// over the parent covers it too, nothing under nothing; under a fold,
    /// its own slot by one look-up.
    pub(super) fn child(self, tree: &Tree, zoom: Zoom, at: [i64; 3]) -> Held {
        match self {
            Held::Folded { block, .. } => Held::of(tree.child(block, slot_of(at)), zoom, (zoom, at)),
            covered => covered,
        }
    }

    /// What a node holding this folds to, as a cell of its zoom is drawn: the
    /// leaf's value, or its fold where half of it or more is filled; none
    /// where nothing that is a voxel is. `hide`, a preview's left-out leaf,
    /// is left out where it is the leaf itself, at its zoom or finer;
    /// coarser, the fold stands as stored.
    pub fn value(self, hide: Option<EntryAt>) -> Option<u32> {
        let v = u32::from(match self {
            Held::Nothing => return None,
            Held::Leaf { leaf, zoom, at } => match hide.is_some_and(|h| h.zoom() == zoom && h.at() == at) {
                true => return None,
                false => leaf.id,
            },
            Held::Folded { fold, .. } => shown(fold),
        });
        object_value_is_voxel(v).then_some(v)
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
            match leaf != EMPTY && object_value_is_voxel(u32::from(leaf.id)) {
                true => Fold { kind: leaf.id, fill: FULL },
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

/// What a folded cube shows: its kind where it is half full or more, so a
/// lone cube goes at the first fold and a wall keeps whole cells, and the
/// volume is rounded once, where it is drawn, not at every zoom on the way.
fn shown(f: Fold) -> u16 {
    if f.fill >= FULL / 2 {
        f.kind
    } else {
        EMPTY.id
    }
}
