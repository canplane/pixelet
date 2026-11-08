// pxl-engine/way/tree.rs

//! gpu-portable: a brick's ways as cells, in a tree of the same shape as the placed tree
//! (pxl-engine object.rs), so a reader asks for them by the one rule it asks what is placed
//! by. The tree runs from the brick's own nodes along z down to its cells. A node's eight
//! slots form a child group, with children only where they differ. Each branch holds its
//! eight folded (`object::fold`), and its leaves hold what the ways write at its cells
//! (`way::way_cell`, read from the brick's spans). A reader asks a brick's ways only of a node
//! of the brick's size or under it; a wider node reads the clip levels' bound over its bricks
//! (`clipmap::bound_over`), so the tree holds nothing above the brick. Its child groups are
//! taken from one pool that every brick's tree shares, and a brick's root word names the
//! child group of its nodes along z (`root_word`, `clipmap::ROOT_AT`), from which a reader
//! steps down as in the placed tree (`cursor`).
//!
//! The tree is built a level at a time, top-down and then back up, in a pool of the brick's
//! own (fill.rs `tree`), and laid in the pool every brick shares once it is complete (`lay`):
//!
//! - the brick's nodes along z that its ways' bound meets are the first level's candidates,
//!   one slot each of its root child group (`span`, `root`);
//! - each candidate coarser than the cells is flagged where its z meets the ways' bound in
//!   the brick's levels (`flag`): a flagged one takes a child group for its children, which
//!   are the next level's candidates (`branch`), and the rest hold nothing (`empty`); a
//!   level's child groups are taken together, after `fits` checks them;
//! - the candidates at the cells are leaves (`leaf`);
//! - from the deepest level up, each branch is folded, and one whose eight are one leaf,
//!   nothing included, is written as that leaf and its child group given back (`fold`). So
//!   the tree held is the one the rule defines, in the fewest child groups that hold it.
//!
//! While the tree is built, a candidate's node is held in its data word, where its leaf or
//! fold is written afterwards (`pack`).

use crate::clipmap::{BrickSource, BRICK_SHIFT};
use crate::fill;
use crate::object::{self, NodeSlot, Table, AIR as PLACED_AIR, BRANCH};
use crate::{ground, Zoom, LEAF_CELL_ZOOM};

/// The tree's columns as its builder writes them, a word a slot.
pub trait Pool: Table {
    fn set_first_child(&mut self, slot: u32, v: u32);
    fn set_data(&mut self, slot: u32, v: u32);
}

/// Where slot `slot`'s child pointer lies in a pool whose child groups lie whole, each as its
/// eight child pointers and then its eight data words, as the core's and a GPU's do
/// (pxl-render world.wgsl).
const fn first_child_at(slot: u32) -> u32 {
    slot / object::CHILDREN * CHILD_GROUP_WORDS + slot % object::CHILDREN
}

/// A pool in one array, with the child groups whole: a brick's own pool as its tree is built
/// (fill.rs `tree`), and a bake's.
pub struct Column<'a>(pub &'a mut [u32]);

impl Table for Column<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot))
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot) + object::CHILDREN)
    }
}

impl Pool for Column<'_> {
    fn set_first_child(&mut self, slot: u32, v: u32) {
        fill::put(self.0, first_child_at(slot), v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        fill::put(self.0, first_child_at(slot) + object::CHILDREN, v);
    }
}

/// A pool in one array as a reader holds it, the child groups whole: the
/// cast's binding, and a test's.
#[derive(Clone, Copy)]
pub struct Read<'a>(pub &'a [u32]);

impl Table for Read<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot))
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot) + object::CHILDREN)
    }
}

/// The most child groups, eight slots each, that the pool shared by every brick's tree grows
/// to. A bake's set fits it wherever the eye is (pxl-spec bake/tile/terrain.rs `pool_needs`),
/// so a pool this large refuses none. The core's pool grows as bricks take it, up to this
/// (pxl-core domain/layer/terrain/pool.rs), and the GPU's follows it (pxl-render
/// gpu/ray/ground.rs `GroundBuffer::grow`); a tree past what the pool holds is refused, and
/// its brick is left absent. Child group 0 is never handed out: its words stay zero, which
/// reads as nothing (`object::Table`), so a root word of 0 is a brick with no tree, read as
/// nothing without a child group of its own (`root_word`).
pub const POOL_CHILD_GROUPS: u32 = 1 << 20;

/// A brick's root word (`clipmap::ROOT_AT`): its root child group's id, which is under
/// `POOL_CHILD_GROUPS`, and above it, in the bits left, the Z of the node the group's first
/// slot holds. It is 0, child group 0, where the brick has no tree.
pub const fn root_word(child_group: u32, z0: i32) -> u32 {
    child_group | (z0 as u32) << ROOT_ID_BITS
}

/// A root word's child group's first slot and its first slot's Z.
pub const fn root_slot(w: u32) -> (u32, i32) {
    (
        object::CHILDREN * (w & (POOL_CHILD_GROUPS - 1)),
        w as i32 >> ROOT_ID_BITS,
    )
}

/// The bits of a root word that hold the child group's id, as many as `POOL_CHILD_GROUPS`
/// takes.
pub const ROOT_ID_BITS: u32 = POOL_CHILD_GROUPS.trailing_zeros();
const _: () = assert!(
    POOL_CHILD_GROUPS.is_power_of_two()
        && (i16::MIN as i32 >> BRICK_SHIFT) >= -(1 << (31 - ROOT_ID_BITS)),
    "a root word holds every child group's id and a brick's Z, the leaves' i16 over its side"
);

/// The most of a brick's own nodes along z that its root child group holds, one slot each. A
/// brick is a node `BRICK_SHIFT` zooms above its cells, and its ways' bound meets no more of
/// them than this. A tile past it fails its bake (pxl-spec bake/tile/terrain.rs `fill_fit`),
/// its build refused as one `past` its child groups is.
pub const ROOT_NODES: u32 = object::CHILDREN;

/// The brick's own nodes along z that its ways' bound `[lo, hi)` (in leaf cells,
/// `ground::Extent`) meets, for cells at `cell_zoom`: the first one's Z and how many, none
/// where the bound is empty.
pub fn span([lo, hi]: [i32; 2], cell_zoom: Zoom) -> (i32, u32) {
    let d = LEAF_CELL_ZOOM - (cell_zoom - BRICK_SHIFT);
    if lo >= hi {
        (0, 0)
    } else {
        (lo >> d, (((hi - 1) >> d) - (lo >> d) + 1) as u32)
    }
}

/// What the node `at` of `zoom`, of the brick's size or under it, holds of the brick's ways'
/// tree whose root word is `root`, with cells at `cell_zoom`. It finds the brick's own node
/// along z above it in the root child group and steps down from there (`object::down`), as
/// `object::cursor` steps; it is nothing where the root holds no node there. A descent asks
/// this once, where it finds the brick (pxl-engine world.rs `node`).
pub fn cursor(
    columns: &impl Table,
    root: u32,
    cell_zoom: Zoom,
    zoom: Zoom,
    at: [i32; 3],
) -> NodeSlot {
    let (first, z0) = root_slot(root);
    let top = cell_zoom - BRICK_SHIFT;
    if root == 0 || zoom < top {
        return NodeSlot::NOTHING;
    }
    let k = (at[2] >> (zoom - top)) - z0;
    if k < 0 || k >= ROOT_NODES as i32 {
        return NodeSlot::NOTHING;
    }
    object::down(columns, first + k as u32, top, zoom, at)
}

/// A child group's words in a pool: its eight child pointers and its eight data
/// words.
pub const CHILD_GROUP_WORDS: u32 = 2 * object::CHILDREN;

/// The most child groups one brick's build takes before its fold gives any back, which is
/// the most the format-12 set takes. A tile past it fails its bake (pxl-geo-bake
/// terrain/pool.rs), and what a build keeps for a brick, its records and the child groups its
/// fold gives back, is held to it.
pub const BRICK_CHILD_GROUPS: u32 = 24_067;

/// No slot: a fold that gives no child group back.
pub const NONE: u32 = u32::MAX;

/// A candidate's word: its X and Y within the brick at its zoom, `BRICK_SHIFT` bits each,
/// and its Z.
pub const REL_MASK: u32 = (1 << BRICK_SHIFT) - 1;
pub const Z_SHIFT: u32 = 2 * BRICK_SHIFT;
const _: () = assert!(
    32 - Z_SHIFT > 16,
    "a cell's Z, an i16 at the leaves, in a candidate's word"
);

/// The candidate word of the node `at`, with its X and Y within the brick.
fn pack(at: [i32; 3]) -> u32 {
    (at[2] as u32) << Z_SHIFT | (at[1] as u32) << BRICK_SHIFT | at[0] as u32
}

/// A candidate word's node, with its X and Y within the brick.
fn unpack(w: u32) -> [i32; 3] {
    [
        (w & REL_MASK) as i32,
        ((w >> BRICK_SHIFT) & REL_MASK) as i32,
        (w as i32) >> Z_SHIFT,
    ]
}

/// The first level: the brick's own nodes along z from `z0`, in the child group
/// at `root` as `cursor` reads them, each a candidate.
pub fn root(pool: &mut impl Pool, z0: i32, root: u32) {
    let mut k = 0;
    while k < ROOT_NODES {
        pool.set_first_child(root + k, 0);
        pool.set_data(root + k, pack([0, 0, z0 + k as i32]));
        k += 1;
    }
}

/// Whether the candidate at `slot`, a node of `zoom`, takes a child group for its children:
/// it does where it is coarser than the brick's cells at `cell_zoom` and its Z meets where the
/// brick's ways write under it, as the ways' bound in the brick's level of its size says
/// (`clipmap::BrickSource::bound_mip`, the brick's slot `s`).
pub fn flag(
    pool: &impl Pool,
    w: &impl BrickSource,
    s: u32,
    cell_zoom: Zoom,
    zoom: Zoom,
    slot: u32,
) -> bool {
    let at = unpack(pool.data(slot));
    if zoom >= cell_zoom {
        return false;
    }
    let e = [at[0] as u32, at[1] as u32];
    let [lo, hi] = ground::extent_of(w.bound_mip(cell_zoom, s, cell_zoom - zoom, e))[1];
    let d = LEAF_CELL_ZOOM - zoom;
    lo < (at[2] + 1) << d && hi > at[2] << d
}

/// Whether a level's child groups, one for each flagged candidate, may be taken together:
/// the pool has `flagged` of its `left` free, and the brick, having taken `used`, stays within
/// `BRICK_CHILD_GROUPS`. Past either, none is taken: the tree is refused and its brick is left
/// absent. The bake bounds both a set's and a brick's (pxl-geo-bake terrain/pool.rs).
pub fn fits(left: u32, used: u32, flagged: u32) -> bool {
    flagged <= left && !past(used, flagged)
}

/// Whether a level's `flagged` child groups take a brick that has taken `used`
/// past its most, `BRICK_CHILD_GROUPS`.
fn past(used: u32, flagged: u32) -> bool {
    flagged > BRICK_CHILD_GROUPS - used
}

/// Makes the flagged candidate at `slot` a branch whose child group begins at `first`; its
/// eight children are the next level's candidates.
pub fn branch(pool: &mut impl Pool, slot: u32, first: u32) {
    let at = unpack(pool.data(slot));
    pool.set_first_child(slot, BRANCH | first);
    let mut k = 0;
    while k < object::CHILDREN {
        let child = [
            2 * at[0] + (k & 1) as i32,
            2 * at[1] + (k >> 1 & 1) as i32,
            2 * at[2] + (k >> 2) as i32,
        ];
        pool.set_first_child(first + k, 0);
        pool.set_data(first + k, pack(child));
        k += 1;
    }
}

/// Marks a candidate that is not flagged as holding nothing.
pub fn empty(pool: &mut impl Pool, slot: u32) {
    pool.set_first_child(slot, 0);
    pool.set_data(slot, ground::VOID);
}

/// Makes the candidate at `slot`, a cell of the brick, a leaf of what the ways write there
/// (`way::way_cell`, from the spans its fill kept), or nothing where they write none. A bore's
/// air is placed air in the tree, so it fills what holds it (`object::voxel`), and reads as
/// the ground's void to a reader, as the ways write it (`way::AIR`).
pub fn leaf(
    pool: &mut impl Pool,
    column_spans: &impl super::SpanTable,
    cell_zoom: Zoom,
    slot: u32,
) {
    let at = unpack(pool.data(slot));
    let val = super::way_cell(column_spans, [at[0], at[1]], cell_zoom, at[2]);
    let word = if val == super::NOTHING {
        ground::VOID
    } else if val == super::AIR {
        object::leaf_word(PLACED_AIR, 0)
    } else {
        object::leaf_word(val, 0)
    };
    pool.set_first_child(slot, 0);
    pool.set_data(slot, word);
}

/// Folds the branch at `slot`, a node of `zoom` whose children are written. Where its eight
/// are leaves of one word, nothing included, it becomes that leaf, and the first slot of the
/// child group it gives back is returned. Otherwise it holds its eight folded by volume
/// (`object::fold`, each child's share `object::share`), and `NONE` is returned.
pub fn fold(pool: &mut impl Pool, zoom: Zoom, slot: u32) -> u32 {
    let first = pool.first_child(slot) & !BRANCH;
    let word = pool.data(first);
    let mut one = true;
    let mut k = 0;
    while k < object::CHILDREN {
        one &= pool.first_child(first + k) == 0 && pool.data(first + k) == word;
        k += 1;
    }
    if one {
        pool.set_first_child(slot, 0);
        pool.set_data(slot, word);
        return first;
    }
    let share = |k: u32| {
        let d = pool.data(first + k);
        if pool.first_child(first + k) == 0 {
            object::share(d)
        } else {
            d
        }
    };
    let folded = object::fold(
        [
            share(0),
            share(1),
            share(2),
            share(3),
            share(4),
            share(5),
            share(6),
            share(7),
        ],
        zoom,
    );
    pool.set_data(slot, folded);
    NONE
}

/// The first slot of the child group under the branch at `slot`, `NONE` under a
/// leaf.
pub fn under(pool: &impl Table, slot: u32) -> u32 {
    let h = pool.first_child(slot);
    if h == 0 {
        NONE
    } else {
        h & !BRANCH
    }
}

/// Lays the tree under root word `root` of `from`, as a brick's build leaves it, into `to`
/// from child group `at`: its child groups in the order a walk from the root meets them, each
/// pointer following its group, and none of those its fold gave back. `order` has room for a
/// word for each group it holds, as the build counts them (fill.rs `tree`). It returns the
/// root word the laid tree is read by.
pub fn lay(from: &impl Table, root: u32, to: &mut impl Pool, at: u32, order: &mut [u32]) -> u32 {
    let (first, z0) = root_slot(root);
    order[0] = first / object::CHILDREN;
    let (mut i, mut n) = (0, 1);
    while i < n {
        let (src, dst) = (
            object::CHILDREN * order[i],
            object::CHILDREN * (at + i as u32),
        );
        let mut k = 0;
        while k < object::CHILDREN {
            let under = under(from, src + k);
            let moved = if under == NONE {
                0
            } else {
                order[n] = under / object::CHILDREN;
                n += 1;
                BRANCH | (object::CHILDREN * (at + n as u32 - 1))
            };
            to.set_first_child(dst + k, moved);
            to.set_data(dst + k, from.data(src + k));
            k += 1;
        }
        i += 1;
    }
    root_word(at, z0)
}

/// How a reader reads a leaf's value: the ground's void for a bore's air, as the ways write
/// it (`way::AIR`), or else the value itself; `ground::VOID` where the tree holds nothing.
pub fn read(val: u32) -> u32 {
    if val == PLACED_AIR {
        super::AIR
    } else {
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_the_bricks_cap() {
        assert!(past(1, BRICK_CHILD_GROUPS));
    }
}
