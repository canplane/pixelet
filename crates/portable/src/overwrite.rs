// pxl-portable/overwrite.rs

//! gpu-portable: what is placed, as a descent asks it — the objects' one
//! tree (pxl-kernel layer/object/store.rs), read where it lives: two lanes
//! of a word a slot, `heap` a branch's children's block with `BRANCH` set or
//! 0 for a leaf, and `data` a leaf's value and its record's handle, or a
//! branch's fold, the kind most of its fill is and the share filled. The
//! root's block holds the nodes of zoom 0, and a node's slot in its parent's
//! block is its index's low bits (`path`). A descent hands each node what its
//! parent found (`Held`), so a node is one look-up.
//!
//! What is placed is the last layer the ground decision lays
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md), so it is
//! asked first (`over`): where it holds a value, air included, that is the
//! block, whatever is under it.

use crate::cube::{object, sentinel, Ground, Object};
use crate::Zoom;

/// A slot's heap word when it is a branch, its children's block under it.
pub const BRANCH: u32 = 1 << 31;

/// The block of the root what is placed is under.
pub const PLACED: u32 = 0;

/// A cube filled whole, as a fold's share counts it: a share is in 2⁻¹⁵ of
/// its cube, floored at each fold, so eighths stay exact five folds deep.
pub const FULL: u32 = 1 << 15;

const VOID: u32 = sentinel::void as u32;
const AIR: u32 = object::air as u32;

/// A cube's eight children folded by volume, the one rule for what a node of
/// an overwrite's tree holds at a coarser zoom, each child as its fold word
/// (`kind` low, its share of its cube filled high, `FULL` whole; a leaf of a
/// voxel fills it whole, one of none fills nothing): the share of the cube
/// filled, the mean of theirs, and the kind most of that fill is, a tie to
/// the corner `majority`'s alternates to (pxl-portable `ground::majority`).
/// The objects' store folds its tree by it as it is written (pxl-kernel
/// layer/object/store.rs).
pub fn fold(children: [u32; 8], zoom: Zoom) -> u32 {
    let (kind, fill) = (|c: u32| c & 0xffff, |c: u32| c >> 16);
    let mut sum = 0;
    let mut k = 0;
    while k < 8 {
        sum += fill(children[k]);
        k += 1;
    }
    let (mut best, mut most) = (0, 0);
    let mut j = 0;
    while j < 8 {
        // The corner order alternates with the zoom, as `majority`'s.
        let c = children[if zoom & 1 == 0 { 7 - j } else { j }];
        let mut w = 0;
        let mut i = 0;
        while i < 8 {
            if kind(children[i]) == kind(c) {
                w += fill(children[i]);
            }
            i += 1;
        }
        if fill(c) > 0 && w > most {
            (best, most) = (kind(c), w);
        }
        j += 1;
    }
    best | (sum / 8) << 16
}

/// The tree's two lanes, a word a slot.
pub trait Lanes {
    fn heap(&self, slot: u32) -> u32;
    fn data(&self, slot: u32) -> u32;
}

/// What a node holds, as a descent hands it down, in one word, since a
/// descent keeps one a zoom: the slot that answers it, whether that slot is a
/// branch, its children's fold (`FOLDED`), or a leaf covering the node whole,
/// and the zoom the slot is at, a leaf's own where it covers finer nodes.
/// Its value is read off the lanes only when asked (`value`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held(u32);

/// A held slot's branch bit, over its zoom.
const FOLDED: u32 = 1 << 5;
const ZOOM_MASK: u32 = FOLDED - 1;
const SLOT_SHIFT: u32 = 6;

impl Held {
    /// Nothing is placed under the node.
    pub const NOTHING: Held = Held(u32::MAX);

    /// As a word a GPU's storage holds.
    pub const fn word(self) -> u32 {
        self.0
    }

    pub const fn of_word(w: u32) -> Held {
        Held(w)
    }

    /// The branch at `slot` of `zoom`.
    pub const fn branch(slot: u32, zoom: Zoom) -> Held {
        Held(slot << SLOT_SHIFT | FOLDED | zoom)
    }

    /// Something is placed under the node.
    pub fn holds(self) -> bool {
        self != Held::NOTHING
    }

    /// Leaves under the node differ: it is their fold.
    pub fn folded(self) -> bool {
        self.holds() && self.0 & FOLDED != 0
    }

    /// The slot that answers the node.
    pub fn slot(self) -> u32 {
        self.0 >> SLOT_SHIFT
    }

    /// The zoom that slot is at.
    pub fn zoom(self) -> Zoom {
        self.0 & ZOOM_MASK
    }
}

/// A preview's entry, by its zoom and index; `NONE` for none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    pub zoom: Zoom,
    pub at: [i32; 3],
}

impl Cut {
    pub const NONE: Cut = Cut { zoom: Zoom::MAX, at: [0; 3] };
}

/// The node `at` of `zoom` as the tree is cut: X counted from the round's
/// start, so the round is the one node of zoom 0 over it and every node is
/// its parent's index doubled, a bit more, on every axis.
pub fn path(zoom: Zoom, at: [i32; 3]) -> [i32; 3] {
    [at[0] & (1u32 << zoom).wrapping_sub(1) as i32, at[1], at[2]]
}

/// A node's slot in its parent's block: X's low bit, then Y's, then Z's.
fn slot_of(at: [i32; 3]) -> u32 {
    (at[0] & 1 | (at[1] & 1) << 1 | (at[2] & 1) << 2) as u32
}

/// What slot `slot` holds, at `zoom`.
fn of(lanes: &impl Lanes, slot: u32, zoom: Zoom) -> Held {
    if lanes.heap(slot) != 0 {
        Held(slot << SLOT_SHIFT | FOLDED | zoom)
    } else if lanes.data(slot) == VOID {
        Held::NOTHING
    } else {
        Held(slot << SLOT_SHIFT | zoom)
    }
}

/// What the node `at` of `zoom` holds with nothing over it known: down from
/// the root whose block is `root` to its own slot, or to the leaf over it. A
/// descent asks this once, at its root.
pub fn cursor(lanes: &impl Lanes, root: u32, zoom: Zoom, at: [i32; 3]) -> Held {
    let p = path(zoom, at);
    let (mut block, mut z) = (root, 0);
    loop {
        let s = zoom - z;
        let held = of(lanes, block + slot_of([p[0] >> s, p[1] >> s, p[2] >> s]), z);
        if !held.folded() || z == zoom {
            return held;
        }
        block = lanes.heap(held.slot()) & !BRANCH;
        z += 1;
    }
}

/// What the child `at` of `zoom` of a node holding `parent` holds: a leaf
/// over the parent covers it too, nothing under nothing; under a fold, its
/// own slot.
pub fn child(lanes: &impl Lanes, parent: Held, zoom: Zoom, at: [i32; 3]) -> Held {
    if parent.folded() {
        of(lanes, (lanes.heap(parent.slot()) & !BRANCH) + slot_of(at), zoom)
    } else {
        parent
    }
}

/// Whether a value is drawn as a voxel, a placeable cube or air, rather than
/// an instance or nothing.
pub const fn voxel(value: u32) -> bool {
    value == AIR || Ground::grounds(value) || Object::objects(value)
}

/// Whether `cut` is at `zoom` or coarser and its cube holds the node `at`.
fn holds(cut: Cut, zoom: Zoom, at: [i32; 3]) -> bool {
    if cut.zoom > zoom {
        return false;
    }
    let s = zoom - cut.zoom;
    let (a, c) = (path(cut.zoom, [at[0] >> s, at[1] >> s, at[2] >> s]), path(cut.zoom, cut.at));
    a[0] == c[0] && a[1] == c[1] && a[2] == c[2]
}

/// What the node `at` of `zoom` holding `held` folds to, as a cell of its zoom
/// is drawn: the leaf's value, or its fold where half of it or more is
/// filled; `VOID` where nothing that is a voxel is. `hide`, a preview's
/// left-out leaf, is left out where it is the leaf itself.
pub fn value(lanes: &impl Lanes, held: Held, hide: Cut, zoom: Zoom, at: [i32; 3]) -> u32 {
    let word = if held.holds() { lanes.data(held.slot()) } else { VOID };
    let v = if !held.holds() {
        VOID
    } else if !held.folded() {
        if hide.zoom == held.zoom() && holds(hide, zoom, at) {
            VOID
        } else {
            word & 0xffff
        }
    } else if word >> 16 >= FULL / 2 {
        word & 0xffff
    } else {
        VOID
    };
    if voxel(v) {
        v
    } else {
        VOID
    }
}

/// The node is for the layers under what is placed to answer.
pub const UNDER: u32 = 0;
/// Something placed under the node may fill it: split it.
pub const SPLIT: u32 = 1;
/// What is placed is the block (`Over::block`).
pub const BLOCK: u32 = 2;

/// What is placed answers for a node, before any layer under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over {
    pub says: u32,
    pub block: u32,
}

/// What is placed answers for the node `at` of `zoom` holding `held`, asked
/// down to `near`: as a block where `near` is its zoom or coarser, the value
/// it holds or `carve`'s air, else the layers under it. Coarser, it answers from its own
/// pyramid as the ground answers from its relief (pxl-portable
/// `ground::node`): the layers under it where nothing placed in it is a
/// block at any zoom asked (`blocks_by`), else a split. A preview's cut and
/// its left-out leaf only make air, so where nothing placed fills, what is
/// under answers and splits where it fills.
pub fn over(lanes: &impl Lanes, held: Held, carve: Cut, hide: Cut, zoom: Zoom, at: [i32; 3], near: Zoom) -> Over {
    if zoom < near {
        let says = if blocks_by(lanes, held, near - zoom) { SPLIT } else { UNDER };
        return Over { says, block: VOID };
    }
    let v = if holds(carve, zoom, at) { AIR } else { value(lanes, held, hide, zoom, at) };
    let says = if v == VOID { UNDER } else { BLOCK };
    Over { says, block: v }
}

/// Whether anything placed under the node holding `held` may be a block at
/// a zoom `d` under it or coarser (`value`'s rule, half its cube filled): a
/// leaf of a voxel fills every cube under it, one of none fills none; a
/// fold's share is the mean of its eight's (spec block/volume.rs
/// `fold_cell`), so the node's filled volume is its share of its cube, and
/// none of the 8ᵈ cubes `d` under it holds half of its own unless the share
/// is at least `FULL / (2·8ᵈ)`, a coarser cube's half more still. A share is
/// floored at each fold, each floor less than one of the fold's units and so
/// an eighth of its parent's, so the share kept is short of the filled
/// volume by less than 8/7 of a unit: two units over it hold it.
fn blocks_by(lanes: &impl Lanes, held: Held, d: Zoom) -> bool {
    if !held.holds() {
        return false;
    }
    let word = lanes.data(held.slot());
    if !held.folded() {
        return voxel(word & 0xffff);
    }
    let half = if 3 * d + 1 < 16 { FULL >> (3 * d + 1) } else { 0 };
    (word >> 16) + 2 > half
}

/// Whether nothing placed or carved is finer than the node `at` of `zoom`
/// holding `held`: none placed under it or one placed covering it whole,
/// and no preview's cut inside it.
pub fn whole(held: Held, carve: Cut, zoom: Zoom, at: [i32; 3]) -> bool {
    let carved = carve.zoom != Zoom::MAX && carve.zoom > zoom && {
        let s = carve.zoom - zoom;
        let c = path(zoom, [carve.at[0] >> s, carve.at[1] >> s, carve.at[2] >> s]);
        let a = path(zoom, at);
        c[0] == a[0] && c[1] == a[1] && c[2] == a[2]
    };
    !held.folded() && !carved
}
