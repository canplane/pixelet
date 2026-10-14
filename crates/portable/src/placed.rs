// pxl-portable/placed.rs

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
/// as a block where `block`: the value it holds or `carve`'s air, else the
/// layers under it; coarser, a split where anything is placed under it.
pub fn over(lanes: &impl Lanes, held: Held, carve: Cut, hide: Cut, zoom: Zoom, at: [i32; 3], block: bool) -> Over {
    if !block {
        let says = if held.holds() { SPLIT } else { UNDER };
        return Over { says, block: VOID };
    }
    let v = if holds(carve, zoom, at) { AIR } else { value(lanes, held, hide, zoom, at) };
    let says = if v == VOID { UNDER } else { BLOCK };
    Over { says, block: v }
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
