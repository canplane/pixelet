// pxl-spec-core/overwrite.rs

//! gpu-portable: what is placed, as a descent asks it — the objects' one
//! tree (pxl-kernel domain/layer/object/store.rs), read where it lives: two lanes
//! of a word a slot, `heap` a branch's children's family with `BRANCH` set or
//! 0 for a leaf, and `data` a leaf's value and its record's handle, or a
//! branch's fold, the kind most of its fill is and the share filled. The
//! root's family holds the nodes of zoom 0, and a node's slot in its parent's
//! family is its index's low bits (`path`). A descent hands each node what its
//! parent found (`Held`), so a node is one look-up.
//!
//! What is placed is the last layer the ground decision lays
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md), so it is
//! asked first (`over`): where it holds a value, air included, that is the
//! block, whatever is under it.

use crate::cube::{object, sentinel, Ground, Object};
use crate::{Zoom, LEAF_CELL};

/// A slot's heap word when it is a branch, its children's family under it.
pub const BRANCH: u32 = 1 << 31;

/// The family of the root what is placed is under.
pub const PLACED: u32 = 0;

pub use crate::answer::FULL;

/// The edits a frame hands what reads them at most (pxl-kernel
/// domain/scene/decor/drawn.rs): each the columns an overwrite wrote, a box
/// of leaf cells `[x0, y0, x1, y1)`, X on any lap; past them the kernel
/// unions the rest into the last.
pub const EDITS: u32 = 8;
/// The boxes of leaf cells a frame's edits and a preview's cuts changed:
/// the edits, and a preview's cut and left-out leaf as they were and as they
/// are; past them the rest is joined into the last (pxl-render gpu/grown.rs
/// `hold`). What reads those columns asks them again: decor's growth, the
/// observed pyramid's forgetting.
pub const BOXES: u32 = EDITS + 4;

/// The leaf cells a preview's entry at `zoom` and `at` covers, as a box.
pub const fn cut_box(zoom: Zoom, at: [i32; 3]) -> [i32; 4] {
    if zoom <= LEAF_CELL {
        let s = LEAF_CELL - zoom;
        [at[0] << s, at[1] << s, (at[0] + 1) << s, (at[1] + 1) << s]
    } else {
        let s = zoom - LEAF_CELL;
        [at[0] >> s, at[1] >> s, (at[0] >> s) + 1, (at[1] >> s) + 1]
    }
}

const VOID: u32 = sentinel::void as u32;
pub const AIR: u32 = object::air as u32;

/// A slot's data word, the one layout every reader of the tree reads it by:
/// a leaf's value low and its record's handle high (`leaf_word`), a
/// branch's fold the kind most of its fill is low and its share of its cube
/// filled high (`fold_word`). Low, the value or the kind either way
/// (`low`); high, the handle or the share (`high`).
pub const fn leaf_word(value: u32, ext: u32) -> u32 {
    value & 0xffff | ext << 16
}

/// A fold's word: its kind and its share (`leaf_word`).
pub const fn fold_word(kind: u32, share: u32) -> u32 {
    kind & 0xffff | share << 16
}

/// A data word's low half: a leaf's value, a fold's kind.
pub const fn low(word: u32) -> u32 {
    word & 0xffff
}

/// A data word's high half: a leaf's handle, a fold's share.
pub const fn high(word: u32) -> u32 {
    word >> 16
}

/// A cube's eight children folded by volume, the one rule for what a node of
/// an overwrite's tree holds at a coarser zoom, each child as its fold word
/// (`kind` low, its share of its cube filled high, `FULL` whole; a leaf of a
/// voxel fills it whole, one of none fills nothing): the share of the cube
/// filled, the mean of theirs, and the kind most of that fill is, a tie to
/// the corner `majority`'s alternates to (pxl-spec-core `ground::majority`).
/// The objects' store folds its tree by it as it is written (pxl-kernel
/// domain/layer/object/store.rs).
pub fn fold(children: [u32; 8], zoom: Zoom) -> u32 {
    let (kind, fill) = (low, high);
    // Child `k`, each read at a constant index: a runtime one is a bounds
    // check, whose panic's path a GPU's loop about it carries out as a flag
    // its own lane's (pxl-shader `add`), so no barrier after it is in
    // uniform control flow.
    let child = |k: u32| match k {
        0 => children[0],
        1 => children[1],
        2 => children[2],
        3 => children[3],
        4 => children[4],
        5 => children[5],
        6 => children[6],
        _ => children[7],
    };
    let mut sum = 0;
    let mut k = 0;
    while k < 8 {
        sum += fill(child(k));
        k += 1;
    }
    let (mut best, mut most) = (0, 0);
    let mut j = 0;
    while j < 8 {
        // The corner order alternates with the zoom, as `majority`'s.
        let c = child(if zoom & 1 == 0 { 7 - j } else { j });
        let mut w = 0;
        let mut i = 0;
        while i < 8 {
            if kind(child(i)) == kind(c) {
                w += fill(child(i));
            }
            i += 1;
        }
        if fill(c) > 0 && w > most {
            (best, most) = (kind(c), w);
        }
        j += 1;
    }
    fold_word(best, sum / 8)
}

/// A leaf's word in its parent's fold (`fold`): a leaf whose value is a
/// voxel fills it whole; the empty word, and any other, none. The one rule
/// whichever side builds a tree, the objects' store (pxl-spec
/// block/volume.rs) or a square's ways (`ways::tree`).
pub fn share(word: u32) -> u32 {
    let id = low(word);
    if word != 0 && voxel(id) {
        fold_word(id, FULL)
    } else {
        0
    }
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
    pub const NONE: Cut = Cut {
        zoom: Zoom::MAX,
        at: [0; 3],
    };
}

/// The node `at` of `zoom` as the tree is cut: X counted from the round's
/// start, so the round is the one node of zoom 0 over it and every node is
/// its parent's index doubled, a bit more, on every axis.
pub fn path(zoom: Zoom, at: [i32; 3]) -> [i32; 3] {
    [at[0] & (1u32 << zoom).wrapping_sub(1) as i32, at[1], at[2]]
}

/// A family's slots, a node's children, each at its `slot_of`.
pub const FAMILY: u32 = 8;

/// A node's slot in its parent's family: X's low bit, then Y's, then Z's.
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
/// the root whose family is `root` to its own slot, or to the leaf over it, a
/// `child` a zoom, the one step every descent of the tree takes. A descent
/// asks this once, at its root.
pub fn cursor(lanes: &impl Lanes, root: u32, zoom: Zoom, at: [i32; 3]) -> Held {
    let p = path(zoom, at);
    let mut held = of(
        lanes,
        root + slot_of([p[0] >> zoom, p[1] >> zoom, p[2] >> zoom]),
        0,
    );
    let mut z = 0;
    while z < zoom && held.folded() {
        z += 1;
        let s = zoom - z;
        held = child(lanes, held, z, [p[0] >> s, p[1] >> s, p[2] >> s]);
    }
    held
}

/// What the child `at` of `zoom` of a node holding `parent` holds: a leaf
/// over the parent covers it too, nothing under nothing; under a fold, its
/// own slot.
pub fn child(lanes: &impl Lanes, parent: Held, zoom: Zoom, at: [i32; 3]) -> Held {
    if parent.folded() {
        of(
            lanes,
            (lanes.heap(parent.slot()) & !BRANCH) + slot_of(at),
            zoom,
        )
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
    let (a, c) = (
        path(cut.zoom, [at[0] >> s, at[1] >> s, at[2] >> s]),
        path(cut.zoom, cut.at),
    );
    a[0] == c[0] && a[1] == c[1] && a[2] == c[2]
}

/// What the node `at` of `zoom` holding `held` folds to, as a cell of its zoom
/// is drawn: the leaf's value, or its fold's kind wherever any of it is
/// filled, with what of its cube that fills (`filled`); `VOID` where nothing
/// that is a voxel is. How much of the block is kept is the block's, of
/// every layer together (`answer::cover`, `answer::eighths`). `hide`, a
/// preview's left-out leaf, is left out where it is the leaf itself.
pub fn value(lanes: &impl Lanes, held: Held, hide: Cut, zoom: Zoom, at: [i32; 3]) -> u32 {
    let word = if held.holds() {
        lanes.data(held.slot())
    } else {
        VOID
    };
    let v = if !held.holds() {
        VOID
    } else if !held.folded() {
        if hide.zoom == held.zoom() && holds(hide, zoom, at) {
            VOID
        } else {
            low(word)
        }
    } else if high(word) > 0 {
        low(word)
    } else {
        VOID
    };
    if voxel(v) {
        v
    } else {
        VOID
    }
}

/// The share of its cube what the node holding `held` folds to fills, in
/// `FULL`'s units (`value`): a leaf over it whole, a fold its share.
pub fn filled(lanes: &impl Lanes, held: Held) -> u32 {
    if held.folded() {
        high(lanes.data(held.slot()))
    } else {
        FULL
    }
}

/// The four looks the node `at` of `zoom` holding `held`, a block of value
/// `v` (`value`), is drawn by, `dx + 2·dy`, as the ground's cell is by the
/// four its fold took (pxl-spec-core `ground::Cell::four`): its 2×2 seen
/// from above one zoom finer, the same for every face as the ground's. A
/// quarter shows its upper child where that fills by `value`'s rule (a
/// leaf's voxel, a fold's kind any of it filled; air does not), else its
/// lower one through it, else the node's own value. A leaf over the node
/// is its value four times, as a ground leaf's cell is. Each child is read
/// at its own index under `hide`, as `v` was, so a leaf a preview leaves
/// out shows in no quarter.
pub fn looks(
    lanes: &impl Lanes,
    held: Held,
    hide: Cut,
    zoom: Zoom,
    at: [i32; 3],
    v: u32,
) -> [u32; 4] {
    if !held.folded() {
        return [v; 4];
    }
    let base = lanes.heap(held.slot()) & !BRANCH;
    let shows = |k: u32| {
        let child = [
            2 * at[0] + (k & 1) as i32,
            2 * at[1] + (k >> 1 & 1) as i32,
            2 * at[2] + (k >> 2) as i32,
        ];
        let c = value(lanes, of(lanes, base + k, zoom + 1), hide, zoom + 1, child);
        (c, c != VOID && c != AIR)
    };
    let mut four = [v; 4];
    let mut j = 0;
    while j < 4 {
        let (upper, up) = shows(j + 4);
        let (lower, low) = shows(j);
        four[j as usize] = if up {
            upper
        } else if low {
            lower
        } else {
            v
        };
        j += 1;
    }
    four
}

/// The node is for the layers under what is placed to answer.
pub const UNDER: u32 = 0;
/// Something placed under the node may fill it: split it.
pub const SPLIT: u32 = 1;
/// What is placed is the block (`Over::block`), filling `Over::fill` of
/// its cube.
pub const BLOCK: u32 = 2;

/// What is placed answers for a node, before any layer under it: what it
/// says, and as a block its value and the share of its cube that fills, in
/// `FULL`'s units (`filled`), whole for a leaf or a cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over {
    pub says: u32,
    pub block: u32,
    pub fill: u32,
}

/// What is placed answers for the node `at` of `zoom` holding `held`, asked
/// down to `near`: as a block where `near` is its zoom or coarser, the value
/// it holds or `carve`'s air and what of its cube that fills, else the
/// layers under it. Coarser, it answers from its own
/// pyramid as the ground answers from its relief (pxl-spec-core
/// `ground::node`): the layers under it where nothing placed in it is a
/// block at any zoom asked (`blocks_by`), else a split. A preview's cut and
/// its left-out leaf only make air, so where nothing placed fills, what is
/// under answers and splits where it fills.
pub fn over(
    lanes: &impl Lanes,
    held: Held,
    carve: Cut,
    hide: Cut,
    zoom: Zoom,
    at: [i32; 3],
    near: Zoom,
) -> Over {
    if zoom < near {
        let says = if blocks_by(lanes, held, near - zoom) {
            SPLIT
        } else {
            UNDER
        };
        return Over {
            says,
            block: VOID,
            fill: 0,
        };
    }
    let carved = holds(carve, zoom, at);
    let v = if carved {
        AIR
    } else {
        value(lanes, held, hide, zoom, at)
    };
    let says = if v == VOID { UNDER } else { BLOCK };
    Over {
        says,
        block: v,
        fill: if carved { FULL } else { filled(lanes, held) },
    }
}

/// Where what is placed under the node holding `held` may lie, as a box in
/// the node's steps on every axis (`answer::box_word`): as its tree holds
/// it, the halves of the node on each axis any of its eight children holds
/// anything in, or all of it where they are one (not folded). None
/// (`answer::BOX_NONE`) where nothing is placed. A preview's cut only makes
/// air, so what is placed under it lies here still.
pub fn bound(lanes: &impl Lanes, held: Held, zoom: Zoom) -> crate::answer::BoxWords {
    use crate::answer::{box_word, BOX_NONE, BOX_STEPS, BOX_WHOLE};
    if !held.holds() {
        return BOX_NONE;
    }
    if !held.folded() {
        return BOX_WHOLE;
    }
    let base = lanes.heap(held.slot()) & !BRANCH;
    // On each axis, whether any child holding anything lies in its lower
    // half, and in its upper: child `k`'s half on axis `a` is bit `a` of `k`.
    // A bit an axis: a GPU's arrays hold no bools.
    let (mut low, mut high) = (0u32, 0u32);
    let mut k = 0;
    while k < 8 {
        if of(lanes, base + k, zoom + 1).holds() {
            let mut a = 0;
            while a < 3 {
                if k >> a & 1 == 0 {
                    low |= 1 << a;
                } else {
                    high |= 1 << a;
                }
                a += 1;
            }
        }
        k += 1;
    }
    if low | high == 0 {
        return BOX_NONE;
    }
    let half = BOX_STEPS / 2;
    let first = |a: u32| if low >> a & 1 != 0 { 0 } else { half };
    let past = |a: u32| if high >> a & 1 != 0 { BOX_STEPS } else { half };
    box_word([first(0), first(1), first(2)], [past(0), past(1), past(2)])
}

/// Whether anything placed under the node holding `held` may be a block at
/// a zoom `d` under it or coarser (a block keeps an eighth of its cube or
/// more, `answer::eighths`): a leaf of a voxel fills every cube under it,
/// one of none fills none; a fold's share is the mean of its eight's (spec
/// block/volume.rs `fold`), so the node's filled volume is its share of
/// its cube, and none of the 8ᵈ cubes `d` under it holds an eighth of its
/// own unless the share is at least `FULL / 8ᵈ⁺¹`, a coarser cube's eighth
/// more still. A share is floored at each fold, each floor less than one of
/// the fold's units and so an eighth of its parent's, so the share kept is
/// short of the filled volume by less than 8/7 of a unit: two units over it
/// hold it. Where the ground fills the rest of such a cube, its own bound
/// splits it.
fn blocks_by(lanes: &impl Lanes, held: Held, d: Zoom) -> bool {
    if !held.holds() {
        return false;
    }
    let word = lanes.data(held.slot());
    if !held.folded() {
        return voxel(low(word));
    }
    let eighth = if 3 * d + 3 < 16 {
        FULL >> (3 * d + 3)
    } else {
        0
    };
    high(word) + 2 > eighth
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::{box_of, BOX_NONE, BOX_WHOLE};

    /// A family at slot 8: the children `held` names hold a voxel, the rest
    /// nothing.
    struct Family(&'static [u32]);

    impl Lanes for Family {
        fn heap(&self, slot: u32) -> u32 {
            if slot == 1 {
                8
            } else {
                0
            }
        }
        fn data(&self, slot: u32) -> u32 {
            if slot >= 8 && self.0.contains(&(slot - 8)) {
                1
            } else {
                VOID
            }
        }
    }

    /// What is placed answers, on each axis, the halves its children hold
    /// anything in; a leaf all of it; nothing, none.
    #[test]
    fn what_is_placed_is_its_children_s_halves_on_every_axis() {
        let zoom = 10;
        let branch = Held::branch(1, zoom);
        // Child 0 (low x, y, z) alone: the low half on every axis.
        assert_eq!(
            box_of(bound(&Family(&[0]), branch, zoom)),
            ([0; 3], [512; 3])
        );
        // Child 5 (x high, y low, z high) alone.
        assert_eq!(
            box_of(bound(&Family(&[5]), branch, zoom)),
            ([512, 0, 512], [1024, 512, 1024])
        );
        // Children 1 and 2: x and y each both halves, z the low.
        assert_eq!(
            box_of(bound(&Family(&[1, 2]), branch, zoom)),
            ([0; 3], [1024, 1024, 512])
        );
        assert_eq!(bound(&Family(&[]), Held::NOTHING, zoom), BOX_NONE);
        assert_eq!(
            bound(&Family(&[]), Held(1 << SLOT_SHIFT | zoom), zoom),
            BOX_WHOLE
        );
    }
}
