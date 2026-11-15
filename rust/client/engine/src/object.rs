// pxl-engine/object.rs

//! wgsl-port: what is placed, as a descent asks it. It is the objects' one tree (pxl-core
//! domain/layer/object/store.rs), read where it lives: two columns of one word a slot.
//! `first_child` holds a branch's child group's first slot with `BRANCH` set, or 0 for a
//! leaf. `data` holds a leaf's value and its record's handle, or a branch's fold: the kind
//! that makes up most of its fill, and the share filled. The root's child group holds the
//! nodes of zoom 0, and a node's slot in its parent's child group is the low bits of its
//! index (`path`). A descent hands each node what its parent found (`NodeSlot`), so each node
//! takes one look-up.
//!
//! What is placed is the last layer laid over the ground, so it is asked first (`over`):
//! where it holds a value, air included, that value is the cell's asset, whatever lies under
//! it.

use crate::answer::{BOX_NONE, BOX_STEPS, BOX_WHOLE, PackedBox, box_word, fills};
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::share;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, Zoom, starts};
use pxl_spec::space::hierarchy::tree::{child_idx, child_of, children};
use pxl_spec::value;

/// The bit of a slot's child pointer that marks a branch; the bits under it hold the
/// branch's child group.
pub const BRANCH: u32 = 1 << 31;

/// The child group of the root that everything placed lies under.
pub const ROOT: u32 = 0;

/// The bits of `FULL`; a share shifted down past them is none.
const FULL_BITS: u32 = FULL.trailing_zeros();

/// The most damage rects the core hands a frame for written objects (pxl-core
/// domain/scene/decor/gpu.rs). Each is the columns an object wrote, as a box of leaf cells
/// `[x0, y0, x1, y1)` with X on any lap. Past this many, the core joins the rest into the last.
pub const WRITE_DAMAGE_CAP: u32 = 8;
/// The most damage rects a frame is asked over: the written objects' (`WRITE_DAMAGE_CAP`) and
/// two for each of a preview's cuts, since each cut that moved gives its box as it was and as
/// it is. Past this many, the rest is joined into the last (pxl-render gpu/generator.rs
/// `hold`). Whatever reads those columns asks them again: decor's generation, and the node
/// cache's forgetting.
pub const DAMAGE_CAP: u32 = WRITE_DAMAGE_CAP + 2 * (CUTS / CUT_WORDS) as u32;

/// The leaf cells a preview's entry at `zoom` and `at` covers, as a box.
pub const fn cut_rect(zoom: Zoom, at: [i32; 3]) -> [i32; 4] {
    if zoom <= LEAF_CELL_ZOOM {
        let s = LEAF_CELL_ZOOM - zoom;
        [at[0] << s, at[1] << s, (at[0] + 1) << s, (at[1] + 1) << s]
    } else {
        let s = zoom - LEAF_CELL_ZOOM;
        [at[0] >> s, at[1] >> s, (at[0] >> s) + 1, (at[1] >> s) + 1]
    }
}

/// A slot's data word, in the one layout every reader of the tree reads. A leaf holds its
/// value in the low half and its record's handle in the high (`leaf_word`). A branch's fold
/// holds, in the low half, the kind that makes up most of its fill, and in the high, the
/// share of its cube filled (`fold_word`). So the low half is the value or the kind (`low`),
/// and the high half the handle or the share (`high`).
pub const fn leaf_word(val: u32, ext: u32) -> u32 {
    val & 0xffff | ext << 16
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

/// A cube's eight children folded by volume: the one rule for what a node of the objects' tree
/// holds at a coarser zoom. Each child comes as its fold word, with its kind low and the share of
/// its cube filled high, `FULL` for whole; a leaf of a voxel fills its cube whole, and a leaf of
/// nothing fills none. The fold's share is the mean of the children's, and its kind is the kind
/// that makes up most of that fill, a tie going to the corner that pxl-spec `ground::majority`
/// alternates to. The objects' store folds its tree by this rule as it is written (pxl-core
/// domain/layer/object/store.rs).
pub fn fold(children: [u32; 8], zoom: Zoom) -> u32 {
    let (kind, fill) = (low, high);
    // Child `k`, each read at a constant index.
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
    let (mut best, mut most) = (Sentinel::Void as u32, 0);
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

/// A leaf's share in its parent's fold (`fold`): a leaf whose value is a voxel fills its cube
/// whole, and the empty word, or any other, fills none. It is the one rule for whichever side
/// builds a tree of what is placed (the objects' store, pxl-core domain/layer/object/volume.rs).
pub fn share(word: u32) -> u32 {
    let id = low(word);
    if voxel(id) { fold_word(id, FULL) } else { 0 }
}

/// The tree's two columns, one word a slot.
pub trait Table {
    fn first_child(&self, slot: u32) -> u32;
    fn data(&self, slot: u32) -> u32;
}

/// What a node holds, as a descent hands it down, in one word, because a descent keeps one
/// for each zoom: the slot that answers it; whether that slot is a branch, which holds its
/// children's fold (`FOLDED`), or a leaf covering the node whole; and the zoom the slot is at,
/// which for a leaf covering finer nodes is the leaf's own. Its value is read from the columns
/// only when asked (`val`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSlot(u32);

/// The bit of a held slot that marks a branch, above its zoom.
pub const FOLDED: u32 = 1 << 5;
pub const ZOOM_MASK: u32 = FOLDED - 1;
pub const SLOT_SHIFT: u32 = FOLDED.trailing_zeros() + 1;

impl NodeSlot {
    /// Nothing is placed under the node.
    pub const NOTHING: NodeSlot = NodeSlot(u32::MAX);

    /// As a word a GPU's storage holds.
    pub const fn word(self) -> u32 {
        self.0
    }

    pub(crate) const fn of_word(w: u32) -> NodeSlot {
        NodeSlot(w)
    }

    /// The branch at `slot` of `zoom`.
    pub const fn branch(slot: u32, zoom: Zoom) -> NodeSlot {
        NodeSlot(slot << SLOT_SHIFT | FOLDED | zoom)
    }

    /// The leaf at `slot` of `zoom`.
    pub const fn leaf(slot: u32, zoom: Zoom) -> NodeSlot {
        NodeSlot(slot << SLOT_SHIFT | zoom)
    }

    /// Something is placed under the node.
    pub fn holds(self) -> bool {
        self != NodeSlot::NOTHING
    }

    /// Whether the leaves under the node differ, so that the slot holds their fold.
    pub fn is_folded(self) -> bool {
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

/// The words of a cut as the eye holds it: X, Y, Z and the zoom (pxl-engine eye.rs `CARVE`,
/// `HIDE`).
pub const CUT_WORDS: usize = 4;
/// Both of a preview's cuts, carve then hide, as one run of words.
pub const CUTS: usize = 2 * CUT_WORDS;

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

/// The node `at` of `zoom` indexed as the tree is cut: X counted from the start of a lap, Y
/// and Z from the world's edge. So the world is the one node of zoom 0, and on every axis a
/// node's index is its parent's doubled, plus one bit.
pub fn path(zoom: Zoom, at: [i32; 3]) -> [i32; 3] {
    [
        at[0] & (1u32 << zoom).wrapping_sub(1) as i32,
        at[1] + starts(zoom)[1],
        at[2] + starts(zoom)[2],
    ]
}

/// A child group's slots, one for each of a node's children, child `child_idx` (pxl-spec
/// tree.rs) lying that many slots past the first.
pub const CHILDREN: u32 = children(3) as u32;

fn of(columns: &impl Table, slot: u32, zoom: Zoom) -> NodeSlot {
    if columns.first_child(slot) != 0 {
        NodeSlot::branch(slot, zoom)
    } else if columns.data(slot) == Sentinel::Void as u32 {
        NodeSlot::NOTHING
    } else {
        NodeSlot::leaf(slot, zoom)
    }
}

/// What the node `at` of `zoom` holds when nothing over it is known: found by descending from
/// the root whose child group is `root` to the node's own slot, or to the leaf over it, one
/// `child` step a zoom, the one step every descent of the tree takes. A descent asks this
/// once, at its root.
pub fn cursor(columns: &impl Table, root: u32, zoom: Zoom, at: [i32; 3]) -> NodeSlot {
    let p = path(zoom, at);
    down(
        columns,
        root + child_idx([p[0] >> zoom, p[1] >> zoom, p[2] >> zoom]) as u32,
        0,
        zoom,
        p,
    )
}

/// What the node `at` of `zoom` holds, found by descending from the node above it at `top`,
/// which `slot` holds, to its own slot or to the leaf over it, one `child` step a zoom.
pub fn down(columns: &impl Table, slot: u32, top: Zoom, zoom: Zoom, at: [i32; 3]) -> NodeSlot {
    let mut node_slot = of(columns, slot, top);
    let mut z = top;
    while z < zoom && node_slot.is_folded() {
        z += 1;
        let s = zoom - z;
        node_slot = child(columns, node_slot, z, [at[0] >> s, at[1] >> s, at[2] >> s]);
    }
    node_slot
}

/// What the child `at` of `zoom` holds, given what its parent holds (`parent`). A leaf over
/// the parent covers the child too, and nothing stays nothing; under a fold, the child has
/// its own slot.
pub fn child(columns: &impl Table, parent: NodeSlot, zoom: Zoom, at: [i32; 3]) -> NodeSlot {
    if parent.is_folded() {
        of(
            columns,
            (columns.first_child(parent.slot()) & !BRANCH) + child_idx(at) as u32,
            zoom,
        )
    } else {
        parent
    }
}

/// Whether a value is drawn as a voxel, a placeable cube or air, rather than
/// an instance or nothing.
pub const fn voxel(val: u32) -> bool {
    val == Sentinel::Air as u32 || value::is_ground(val) || value::is_object(val)
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

/// What the node `at` of `zoom` holding `slot` folds to, as a cell of its zoom is drawn: the
/// leaf's value, or the fold's kind wherever any of it is filled, filling the share of its
/// cube that `fill_share` gives; `Sentinel::Void` where nothing there is a voxel. How much of
/// the cell is kept is decided for the cell, over every layer together (`answer::cover`,
/// `share::eighths`). `hide`, a preview's left-out leaf, is left out where it is the leaf
/// itself.
pub fn val(columns: &impl Table, slot: NodeSlot, hide: Cut, zoom: Zoom, at: [i32; 3]) -> u32 {
    let word = if slot.holds() {
        columns.data(slot.slot())
    } else {
        Sentinel::Void as u32
    };
    let v = if !slot.holds() {
        Sentinel::Void as u32
    } else if !slot.is_folded() {
        if hide.zoom == slot.zoom() && holds(hide, zoom, at) {
            Sentinel::Void as u32
        } else {
            low(word)
        }
    } else if high(word) > 0 {
        low(word)
    } else {
        Sentinel::Void as u32
    };
    if voxel(v) { v } else { Sentinel::Void as u32 }
}

/// The share of its cube that what the node holding `slot` folds to fills, in `FULL`'s units
/// (`val`): whole for a leaf over it, and the fold's share for a fold.
pub fn fill_share(columns: &impl Table, slot: NodeSlot) -> u32 {
    if slot.is_folded() {
        high(columns.data(slot.slot()))
    } else {
        FULL
    }
}

/// The four asset ids, in the order `dx + 2·dy`, that the node `at` of `zoom` holding
/// `slot`, a cell of value `v` (`val`), is drawn by. They are taken as the ground's cell takes
/// the four of its fold (pxl-engine `ground::Cell::four`): the node's 2×2 seen from above one
/// zoom finer, the same for every face, as the ground's are. A quarter shows its upper child
/// where that child fills by `val`'s rule (a leaf's voxel, or a fold's kind where any of it is
/// filled; air does not fill), or else its lower child seen through it, or else the node's
/// own value. A leaf over the node gives its value four times, as a ground leaf's cell does.
/// Each child is read at its own index under `hide`, as `v` was, so a leaf a preview leaves
/// out shows in no quarter.
pub fn asset_ids(
    columns: &impl Table,
    slot: NodeSlot,
    hide: Cut,
    zoom: Zoom,
    at: [i32; 3],
    v: u32,
) -> [u32; 4] {
    if !slot.is_folded() {
        return [v; 4];
    }
    let base = columns.first_child(slot.slot()) & !BRANCH;
    let shows = |k: u32| {
        let child = child_of(at, k as usize);
        let c = val(
            columns,
            of(columns, base + k, zoom + 1),
            hide,
            zoom + 1,
            child,
        );
        (c, fills(c))
    };
    // Each quarter's upper child, then its lower one, one child a step, so each child is asked
    // once.
    let mut four = [v; 4];
    let mut upper = (Sentinel::Void as u32, false);
    let mut i = 0;
    while i < 2 * 4 {
        let j = i / 2;
        let (c, shown) = shows(if i % 2 == 0 { j + 4 } else { j });
        if i % 2 == 0 {
            upper = (c, shown);
        } else {
            four[j as usize] = if upper.1 {
                upper.0
            } else if shown {
                c
            } else {
                v
            };
        }
        i += 1;
    }
    four
}

/// The node is for the layers under what is placed to answer.
pub const UNDER: u32 = 0;
/// Something placed under the node may fill it: split it.
pub(crate) const SPLIT: u32 = 1;
/// What is placed is the cell's asset (`Over::asset`), filling `Over::fill` of
/// its cube.
pub const CELL: u32 = 2;

/// What is placed answers for a node, before any layer under it: what it says (`UNDER`,
/// `SPLIT` or `CELL`), and as a cell, its value and the share of its cube that fills, in
/// `FULL`'s units (`fill_share`), whole for a leaf or a cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over {
    pub answer: u32,
    pub asset: u32,
    pub fill: u32,
}

/// What is placed answers for the node `at` of `zoom` holding `slot`, asked down to `near`.
/// Where `near` is the node's zoom or coarser, it answers as a cell: the value it holds, or
/// `carve`'s air, and the share of its cube that fills; or else it leaves the node to the
/// layers under it. At a coarser node it answers from its own pyramid, as the ground answers
/// from its relief (pxl-engine `ground::node`): it leaves the node to the layers under it
/// where nothing placed in it is a cell at any zoom asked (`cells_by`), and splits it
/// otherwise. A preview's cut and its left-out leaf only make air, so where nothing placed
/// fills, what lies under answers, and splits where it fills.
pub fn over(
    columns: &impl Table,
    slot: NodeSlot,
    carve: Cut,
    hide: Cut,
    zoom: Zoom,
    at: [i32; 3],
    near: Zoom,
) -> Over {
    if zoom < near {
        let answer = if cells_by(columns, slot, near - zoom) {
            SPLIT
        } else {
            UNDER
        };
        return Over {
            answer,
            asset: Sentinel::Void as u32,
            fill: 0,
        };
    }
    let carved = holds(carve, zoom, at);
    let v = if carved {
        Sentinel::Air as u32
    } else {
        val(columns, slot, hide, zoom, at)
    };
    let answer = if v == Sentinel::Void as u32 {
        UNDER
    } else {
        CELL
    };
    Over {
        answer,
        asset: v,
        fill: if carved {
            FULL
        } else {
            fill_share(columns, slot)
        },
    }
}

/// Where what is placed under the node holding `slot` may lie, as a box in the node's steps
/// on every axis (`answer::box_word`). As its tree holds it, the box covers, on each axis, the
/// halves of the node in which any of its eight children holds anything, or the whole node
/// where the children are one (not folded). It is `answer::BOX_NONE` where nothing is placed.
/// A preview's cut only makes air, so what is placed under the cut still lies in the box.
pub fn object_box(columns: &impl Table, slot: NodeSlot, zoom: Zoom) -> PackedBox {
    if !slot.holds() {
        return BOX_NONE;
    }
    if !slot.is_folded() {
        return BOX_WHOLE;
    }
    let base = columns.first_child(slot.slot()) & !BRANCH;
    // On each axis, whether any child holding anything lies in the lower half, and whether any
    // lies in the upper: child `k`'s half on axis `a` is bit `a` of `k`. They are kept as one
    // bit an axis because a GPU's arrays hold no bools.
    let (mut low, mut high) = (0u32, 0u32);
    let mut k = 0;
    while k < 8 {
        if of(columns, base + k, zoom + 1).holds() {
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

/// Whether anything placed under the node holding `slot` may be a cell at a zoom `d` under
/// it or coarser; a cell keeps an eighth of its cube or more (`share::eighths`). A leaf of a
/// voxel fills every cube under it, and a leaf of nothing fills none. A fold's share is the
/// mean of its eight children's (pxl-core domain/layer/object/volume.rs `fold`), so the node's
/// filled volume is its share of its cube. None of the 8ᵈ cubes `d` zooms under it holds an eighth
/// of its own unless the share is at least `FULL / 8ᵈ⁺¹`, and a coarser cube's eighth needs
/// more still. A share is floored at each fold, each floor losing less than one of the fold's
/// units, which is an eighth of a unit of its parent's, so the share left falls short of the
/// filled volume by less than 8/7 of a unit; a margin of two units covers it. Where the ground
/// fills the rest of such a cube, the ground's own bound splits it.
fn cells_by(columns: &impl Table, slot: NodeSlot, d: Zoom) -> bool {
    if !slot.holds() {
        return false;
    }
    let word = columns.data(slot.slot());
    if !slot.is_folded() {
        return voxel(low(word));
    }
    let shift = share::FILL_BITS * (d + 1);
    let eighth = if shift <= FULL_BITS { FULL >> shift } else { 0 };
    high(word) + 2 > eighth
}

/// Whether nothing placed or carved is finer than the node `at` of `zoom` holding `slot`:
/// nothing is placed under it, or one leaf placed covers it whole, and no preview's cut lies
/// inside it.
pub fn whole(slot: NodeSlot, carve: Cut, zoom: Zoom, at: [i32; 3]) -> bool {
    let carved = carve.zoom != Zoom::MAX && carve.zoom > zoom && {
        let s = carve.zoom - zoom;
        let c = path(zoom, [carve.at[0] >> s, carve.at[1] >> s, carve.at[2] >> s]);
        let a = path(zoom, at);
        c[0] == a[0] && c[1] == a[1] && c[2] == a[2]
    };
    !slot.is_folded() && !carved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::box_of;

    /// A child group at slot 8: the children it names hold a voxel, the rest
    /// nothing.
    struct ChildGroup(&'static [u32]);

    impl Table for ChildGroup {
        fn first_child(&self, slot: u32) -> u32 {
            if slot == 1 { 8 } else { 0 }
        }
        fn data(&self, slot: u32) -> u32 {
            if slot >= 8 && self.0.contains(&(slot - 8)) {
                1
            } else {
                Sentinel::Void as u32
            }
        }
    }

    /// What is placed answers, on each axis, the halves in which its children hold anything; a
    /// leaf answers the whole node, and nothing answers none.
    #[test]
    fn placed_is_child_halves() {
        let zoom = 10;
        let branch = NodeSlot::branch(1, zoom);
        // Child 0 (low x, y, z) alone: the low half on every axis.
        assert_eq!(
            box_of(object_box(&ChildGroup(&[0]), branch, zoom)),
            ([0; 3], [512; 3])
        );
        // Child 5 (x high, y low, z high) alone.
        assert_eq!(
            box_of(object_box(&ChildGroup(&[5]), branch, zoom)),
            ([512, 0, 512], [1024, 512, 1024])
        );
        // Children 1 and 2: x and y each both halves, z the low.
        assert_eq!(
            box_of(object_box(&ChildGroup(&[1, 2]), branch, zoom)),
            ([0; 3], [1024, 1024, 512])
        );
        assert_eq!(
            object_box(&ChildGroup(&[]), NodeSlot::NOTHING, zoom),
            BOX_NONE
        );
        assert_eq!(
            object_box(&ChildGroup(&[]), NodeSlot::leaf(1, zoom), zoom),
            BOX_WHOLE
        );
    }
}
