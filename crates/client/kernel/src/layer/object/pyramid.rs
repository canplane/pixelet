// pxl-kernel/layer/object/pyramid.rs

//! The objects' pyramid: what each node holds at every zoom, kept beside the
//! store's one record of what was placed and derived from it, as the bake
//! folds the ground's tiles a zoom at a time. A leaf placed is its value; a
//! node over leaves that differ is its eight children folded (`fold_cell`,
//! the majority of their fill). The objects are not fetched today, so the
//! layer folds its own pyramid from its writes, where a served tile would
//! otherwise arrive: a write refolds the nodes over it, zoom by zoom, and
//! nothing else. A reader asks a node by one look-up, what covers it handed
//! down from its parent (`Held`), and never folds what is under it.

use fnv::FnvHashMap;

use pxl_spec::block::object::object_value_is_voxel;
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::{Zoom, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::child_cube;
use pxl_spec::space::hierarchy::sparse_tree::Slot;

use super::store::{fold_cell, shard, shown, EntryAt, Leaf, Tree, EMPTY};

/// What a node holds, as a descent hands it down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    /// Nothing is placed under it.
    Nothing,
    /// A leaf placed at `zoom`, `at` covers it whole.
    Leaf { leaf: Leaf, zoom: Zoom, at: [i64; 3] },
    /// Leaves under it differ: their fold, the kind most of its fill is and
    /// the share filled.
    Folded(u16, f64),
}

impl Held {
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
            Held::Folded(kind, fill) => shown((kind, fill)),
        });
        object_value_is_voxel(v).then_some(v)
    }
}

/// A node's kept record, by its zoom and index: a leaf placed there, or
/// the fold of the eight under it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Record {
    Leaf(Leaf),
    Folded(u16, f64),
}

#[derive(Default)]
pub struct Pyramid {
    records: FnvHashMap<(Zoom, [i64; 3]), Record>,
}

impl Pyramid {
    /// What the node at `zoom`, `at` holds, X as the world names it, with
    /// nothing over it known: its own record, or a leaf over it, found up its
    /// line. A descent asks this once, at its root.
    pub fn held(&self, zoom: Zoom, at: [i64; 3]) -> Held {
        let mut z = zoom;
        loop {
            let a = wrap_cell(z, at.map(|v| v >> (zoom - z)));
            match self.records.get(&(z, a)) {
                Some(Record::Leaf(leaf)) => return Held::Leaf { leaf: *leaf, zoom: z, at: a },
                Some(Record::Folded(kind, fill)) if z == zoom => return Held::Folded(*kind, *fill),
                // Over it the leaves differ; under it nothing is kept: nothing.
                Some(Record::Folded(..)) => return Held::Nothing,
                None if z == 0 => return Held::Nothing,
                None => z -= 1,
            }
        }
    }

    /// What the child at `zoom`, `at` of a node holding `parent` holds: a leaf
    /// over the parent covers it too, nothing under nothing; under a fold,
    /// its own record by one look-up.
    pub fn child(&self, parent: Held, zoom: Zoom, at: [i64; 3]) -> Held {
        match parent {
            Held::Folded(..) => match self.records.get(&(zoom, at)) {
                Some(Record::Leaf(leaf)) => Held::Leaf { leaf: *leaf, zoom, at },
                Some(Record::Folded(kind, fill)) => Held::Folded(*kind, *fill),
                None => Held::Nothing,
            },
            covered => covered,
        }
    }

    /// The records under the node at `zoom`, `at` of `tree`, as it stands
    /// before a write replaces it, let go.
    pub(super) fn forget(&mut self, tree: Option<&Tree>, zoom: Zoom, at: [i64; 3]) {
        let Some(tree) = tree else { return };
        let (depth, slot) = match zoom {
            SHARD_ZOOM => (0, Slot::Branch(0)),
            _ => tree.get(u32::from(zoom - SHARD_ZOOM), at),
        };
        // Over it a leaf: it holds no records of its own.
        if SHARD_ZOOM + depth as Zoom != zoom {
            return;
        }
        self.forget_under(tree, slot, zoom, at);
    }

    fn forget_under(&mut self, tree: &Tree, slot: Slot<Leaf>, zoom: Zoom, at: [i64; 3]) {
        self.records.remove(&(zoom, at));
        if let Slot::Branch(block) = slot {
            for (k, s) in tree.block(block).enumerate() {
                self.forget_under(tree, s, zoom + 1, child_cube(zoom, at, k));
            }
        }
    }

    /// The records on the line from its shard's root to the node at `zoom`,
    /// `at` of `tree` as it stands after a write there, made again: each
    /// level's eight as the tree holds them, a branch the fold of its eight,
    /// from the deepest up. What the tree joined over the node is let go.
    pub(super) fn refold(&mut self, tree: Option<&Tree>, zoom: Zoom, at: [i64; 3]) {
        let root = shard(zoom, at);
        let mut block = tree.map(|_| 0u32);
        let mut line = [(0 as Zoom, [0i64; 3]); 64];
        let mut n = 0;
        for z in CUBE_ZOOM..=zoom {
            let parent = match z {
                CUBE_ZOOM => root,
                _ => at.map(|v| v >> (zoom - (z - 1))),
            };
            let slots: [Slot<Leaf>; 8] = match (tree, block) {
                (Some(t), Some(b)) => core::array::from_fn(|k| t.block(b).nth(k).unwrap_or(Slot::Leaf(EMPTY))),
                // Joined over this level, or no tree: nothing kept here.
                _ => [Slot::Leaf(EMPTY); 8],
            };
            let mine = at.map(|v| v >> (zoom - z));
            block = None;
            for (k, s) in slots.into_iter().enumerate() {
                let c = child_cube(z - 1, parent, k);
                match s {
                    Slot::Leaf(v) if v == EMPTY => {
                        self.records.remove(&(z, c));
                    }
                    Slot::Leaf(v) => {
                        self.records.insert((z, c), Record::Leaf(v));
                    }
                    Slot::Branch(b) if c == mine => block = Some(b),
                    // A branch off the line is as it was.
                    Slot::Branch(_) => {}
                }
            }
            if block.is_some() {
                line[n] = (z, mine);
                n += 1;
            }
        }
        for &(z, c) in line[..n].iter().rev() {
            let kids: [(u16, f64); 8] = core::array::from_fn(|k| self.records.get(&(z + 1, child_cube(z, c, k))).map_or((EMPTY.id, 0.0), |r| cell(*r)));
            let (kind, fill) = fold_cell(kids, z);
            self.records.insert((z, c), Record::Folded(kind, fill));
        }
        // Over the shard's cubes, up to the world's roots: each node the fold
        // of what is under it, none where nothing is. A node's X is named as
        // the world names it at its own zoom (`wrap_cell`), so the root at zoom
        // 0 is the whole round, both halves of it under it.
        for z in (0..CUBE_ZOOM).rev() {
            let c = wrap_cell(z, at.map(|v| v >> (zoom - z)));
            let kids: [Option<Record>; 8] = core::array::from_fn(|k| self.records.get(&(z + 1, wrap_cell(z + 1, child_cube(z, c, k)))).copied());
            if kids.iter().all(Option::is_none) {
                self.records.remove(&(z, c));
            } else {
                let (kind, fill) = fold_cell(kids.map(|r| r.map_or((EMPTY.id, 0.0), cell)), z);
                self.records.insert((z, c), Record::Folded(kind, fill));
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.records.clear();
    }
}

/// A child's share in its parent's fold: a placed voxel fills it whole.
fn cell(r: Record) -> (u16, f64) {
    match r {
        Record::Leaf(v) if v != EMPTY && object_value_is_voxel(u32::from(v.id)) => (v.id, 1.0),
        Record::Leaf(_) => (EMPTY.id, 0.0),
        Record::Folded(kind, fill) => (kind, fill),
    }
}
