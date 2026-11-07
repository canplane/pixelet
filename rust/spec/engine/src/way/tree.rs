// pxl-engine/way/tree.rs

//! gpu-portable: a brick's ways as cells, in a tree of the placed tree's
//! shape (pxl-engine overwrite.rs), so a reader asks them by the one
//! rule it asks what is placed by: from the brick's own nodes along z down
//! to its cells, a node's eight slots a child group, children only where they
//! differ, each branch its eight folded (`overwrite::fold`), its leaves what
//! the ways write at its cells (`way::way_cell`, read off the brick's spans). A
//! reader asks a brick's ways only of a node the brick's or under it, a
//! wider one the clip levels' bound over its bricks (`clipmap::bound_over`), so
//! the tree holds nothing over the brick. Its child groups are taken from one
//! pool every brick's tree shares, and a brick's root word names the child
//! group of its nodes along z (`root_word`, `clipmap::ROOT_AT`), from which a
//! reader steps down as the placed tree's does (`cursor`).
//!
//! Built a level at a time, top-down then back up, in a pool of the brick's
//! own (fill.rs `tree`), and laid in the pool every brick shares once it holds
//! what it holds (`lay`):
//!
//! - the brick's nodes along z its ways' bound meets are the first
//!   level's candidates, a slot each of its root child group (`span`, `root`);
//! - each candidate coarser than the cells is flagged where its z meets
//!   the ways' bound in the brick's levels (`flag`): a flagged one takes a
//!   child group for its children,
//!   the next level's candidates (`branch`), the rest hold nothing
//!   (`empty`); a level's child groups are taken together, `fits` the check
//!   before;
//! - the candidates at the cells are leaves (`leaf`);
//! - from the deepest level up, each branch is folded, and one whose eight
//!   are one leaf, nothing included, is written as that leaf and its child group
//!   given back (`fold`): the tree held is the one the rule defines, its
//!   child groups the fewest that hold it.
//!
//! A candidate's node is held in its data word while the tree is built,
//! where its leaf or fold is written after (`pack`).

use crate::clipmap::{Bricks, BRICK_SHIFT};
use crate::fill;
use crate::overwrite::{self, Columns, NodeSlot, AIR as PLACED_AIR, BRANCH};
use crate::{ground, Zoom, LEAF_CELL};

/// The tree's columns as its builder writes them, a word a slot.
pub trait Pool: Columns {
    fn set_first_child(&mut self, slot: u32, v: u32);
    fn set_data(&mut self, slot: u32, v: u32);
}

/// Word `slot`'s child pointer's place in a pool whose child groups lie whole,
/// its eight child pointers then its eight data words, as the kernel's
/// and a GPU's do (pxl-render world.wgsl).
const fn first_child_at(slot: u32) -> u32 {
    slot / overwrite::CHILDREN * CHILD_GROUP_WORDS + slot % overwrite::CHILDREN
}

/// A pool in one array, the child groups whole, a brick's own as it is built
/// (fill.rs `tree`) and a bake's.
pub struct Column<'a>(pub &'a mut [u32]);

impl Columns for Column<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot))
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot) + overwrite::CHILDREN)
    }
}

impl Pool for Column<'_> {
    fn set_first_child(&mut self, slot: u32, v: u32) {
        fill::put(self.0, first_child_at(slot), v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        fill::put(self.0, first_child_at(slot) + overwrite::CHILDREN, v);
    }
}

/// A pool in one array as a reader holds it, the child groups whole: the
/// cast's binding, and a test's.
#[derive(Clone, Copy)]
pub struct Read<'a>(pub &'a [u32]);

impl Columns for Read<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot))
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, first_child_at(slot) + overwrite::CHILDREN)
    }
}

/// The most child groups the pool grows to, eight slots each, every brick's
/// tree's: a bake's set fits it wherever the eye is (pxl-spec
/// bake/tile/terrain.rs `pool_needs`), so a pool this large refuses none.
/// The kernel's pool grows as bricks take it, up to this (pxl-kernel
/// domain/layer/terrain/pool.rs), the GPU's following it (pxl-render
/// gpu/ray/ground.rs `GroundBuffers::grow`); a tree past what it holds is
/// refused, its brick left absent. Child group 0
/// is never handed out: its words stay zero, nothing (`overwrite::Columns`),
/// so a root word 0 is a brick with no tree, read as nothing without a
/// child group of its own (`root_word`).
pub const POOL_CHILD_GROUPS: u32 = 1 << 20;

/// A brick's root word (`clipmap::ROOT_AT`): its root child group's id, under
/// `POOL_CHILD_GROUPS`, and over it the Z of the node its first slot holds, in
/// the bits left; 0, child group 0, where the brick has no tree.
pub const fn root_word(child_group: u32, z0: i32) -> u32 {
    child_group | (z0 as u32) << ROOT_ID_BITS
}

/// A root word's child group's first slot and its first slot's Z.
pub const fn root_slot(w: u32) -> (u32, i32) {
    (
        overwrite::CHILDREN * (w & (POOL_CHILD_GROUPS - 1)),
        w as i32 >> ROOT_ID_BITS,
    )
}

/// A root word's child group id bits, `POOL_CHILD_GROUPS`'s.
pub const ROOT_ID_BITS: u32 = POOL_CHILD_GROUPS.trailing_zeros();
const _: () = assert!(
    POOL_CHILD_GROUPS.is_power_of_two()
        && (i16::MIN as i32 >> BRICK_SHIFT) >= -(1 << (31 - ROOT_ID_BITS)),
    "a root word holds every child group's id and a brick's Z, the leaves' i16 over its side"
);

/// The most of a brick's own nodes along z its root child group holds, a slot
/// each: a brick is a node `BRICK_SHIFT` zooms over its cells, and its
/// ways' bound meets no more of them than this; a tile past it fails its
/// bake (pxl-spec bake/tile/terrain.rs `fill_fit`), its build refused as one
/// `past` its child groups.
pub const ROOT_NODES: u32 = overwrite::CHILDREN;

/// The brick's own nodes along z its ways' bound `[lo, hi)` (leaf cells,
/// `ground::Bounds`) meets, its cells at `cell`: the first's Z and how
/// many, none where the bound is empty.
pub fn span([lo, hi]: [i32; 2], cell: Zoom) -> (i32, u32) {
    let d = LEAF_CELL - (cell - BRICK_SHIFT);
    if lo >= hi {
        (0, 0)
    } else {
        (lo >> d, (((hi - 1) >> d) - (lo >> d) + 1) as u32)
    }
}

/// What the node `at` of `zoom` holds of the brick's ways' tree whose root
/// word is `root`, its cells at `cell`, the node the brick's or under it:
/// the brick's own node along z over it in the root child group, then down
/// (`overwrite::down`), as `overwrite::cursor` steps; nothing where the root
/// holds no node there. A descent asks this once, where it finds the
/// brick (pxl-engine world.rs `node`).
pub fn cursor(columns: &impl Columns, root: u32, cell: Zoom, zoom: Zoom, at: [i32; 3]) -> NodeSlot {
    let (first, z0) = root_slot(root);
    let top = cell - BRICK_SHIFT;
    if root == 0 || zoom < top {
        return NodeSlot::NOTHING;
    }
    let k = (at[2] >> (zoom - top)) - z0;
    if k < 0 || k >= ROOT_NODES as i32 {
        return NodeSlot::NOTHING;
    }
    overwrite::down(columns, first + k as u32, top, zoom, at)
}

/// A child group's words in a pool: its eight child pointers and its eight data
/// words.
pub const CHILD_GROUP_WORDS: u32 = 2 * overwrite::CHILDREN;

/// The most child groups one brick's build takes before its fold gives any
/// back: the most of the format-12 set; a tile past it
/// fails its bake (pxl-geo-bake terrain/pool.rs), and what a build keeps a
/// brick, its records and the child groups its fold gives back, is held to it.
pub const BRICK_CHILD_GROUPS: u32 = 24_067;

/// No slot: a fold that gives no child group back.
pub const NONE: u32 = u32::MAX;

/// A candidate's word: its X and Y in the brick at its zoom,
/// `BRICK_SHIFT` bits each, and its Z.
pub const REL_MASK: u32 = (1 << BRICK_SHIFT) - 1;
pub const Z_SHIFT: u32 = 2 * BRICK_SHIFT;
const _: () = assert!(
    32 - Z_SHIFT > 16,
    "a cell's Z, an i16 at the leaves, in a candidate's word"
);

/// The candidate word of the node `at`, its X and Y in the brick.
fn pack(at: [i32; 3]) -> u32 {
    (at[2] as u32) << Z_SHIFT | (at[1] as u32) << BRICK_SHIFT | at[0] as u32
}

/// A candidate word's node, its X and Y in the brick.
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

/// Whether the candidate at `slot`, a node of `zoom`, takes a child group for
/// its children: it is coarser than the brick's cells at `cell`, and its
/// Z meets where the brick's ways write under it, the ways' bound of the
/// brick's levels at its size (`clipmap::Bricks::bound_level`, its slot `s`).
pub fn flag(pool: &impl Pool, w: &impl Bricks, s: u32, cell: Zoom, zoom: Zoom, slot: u32) -> bool {
    let at = unpack(pool.data(slot));
    if zoom >= cell {
        return false;
    }
    let e = [at[0] as u32, at[1] as u32];
    let [lo, hi] = ground::bounds_of(w.bound_level(cell, s, cell - zoom, e))[1];
    let d = LEAF_CELL - zoom;
    lo < (at[2] + 1) << d && hi > at[2] << d
}

/// Whether a level's child groups, a flagged candidate's each, may be taken
/// together: the pool has `flagged` left of `left`, and the brick, having
/// taken `taken`, stays within `BRICK_CHILD_GROUPS`. None is taken past either:
/// the tree is refused, its brick absent. The bake bounds a set's and a
/// brick's (pxl-geo-bake terrain/pool.rs).
pub fn fits(left: u32, taken: u32, flagged: u32) -> bool {
    flagged <= left && !past(taken, flagged)
}

/// Whether a level's `flagged` child groups take a brick that has taken `taken`
/// past its most, `BRICK_CHILD_GROUPS`.
fn past(taken: u32, flagged: u32) -> bool {
    flagged > BRICK_CHILD_GROUPS - taken
}

/// The flagged candidate at `slot`, a branch whose child group begins
/// at `first`: its eight, the next level's candidates.
pub fn branch(pool: &mut impl Pool, slot: u32, first: u32) {
    let at = unpack(pool.data(slot));
    pool.set_first_child(slot, BRANCH | first);
    let mut k = 0;
    while k < overwrite::CHILDREN {
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

/// A candidate not flagged: nothing under it.
pub fn empty(pool: &mut impl Pool, slot: u32) {
    pool.set_first_child(slot, 0);
    pool.set_data(slot, ground::VOID);
}

/// The candidate at `slot`, a cell of the brick: a leaf of what the ways
/// write there (`way::way_cell`, off the spans its fill kept), nothing where
/// they write none. A bore's air is placed air in the tree, filling what it
/// holds (`overwrite::voxel`), and the ground's void where a reader reads
/// it, as the ways write it (`way::AIR`).
pub fn leaf(pool: &mut impl Pool, column_spans: &impl super::ColumnSpans, cell: Zoom, slot: u32) {
    let at = unpack(pool.data(slot));
    let val = super::way_cell(column_spans, [at[0], at[1]], cell, at[2]);
    let word = if val == super::NOTHING {
        ground::VOID
    } else if val == super::AIR {
        overwrite::leaf_word(PLACED_AIR, 0)
    } else {
        overwrite::leaf_word(val, 0)
    };
    pool.set_first_child(slot, 0);
    pool.set_data(slot, word);
}

/// The branch at `slot`, a node of `zoom`, its children written: where its
/// eight are leaves of one word, nothing included, that leaf, and the first
/// slot of the child group it gives back; else its eight folded by volume
/// (`overwrite::fold`, each child's share `overwrite::share`), and `NONE`.
pub fn fold(pool: &mut impl Pool, zoom: Zoom, slot: u32) -> u32 {
    let first = pool.first_child(slot) & !BRANCH;
    let word = pool.data(first);
    let mut one = true;
    let mut k = 0;
    while k < overwrite::CHILDREN {
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
            overwrite::share(d)
        } else {
            d
        }
    };
    let folded = overwrite::fold(
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
pub fn under(pool: &impl Columns, slot: u32) -> u32 {
    let h = pool.first_child(slot);
    if h == 0 {
        NONE
    } else {
        h & !BRANCH
    }
}

/// The tree under root word `root` of `from`, as a brick's build leaves it,
/// laid into `to` from child group `at`: its child groups in the order a
/// walk from the root meets them, each pointer following it, none of what its
/// fold gave back among them. `order` has room for a word of each it holds, as
/// the build counts them (fill.rs `tree`). The root word it is then read by.
pub fn lay(from: &impl Columns, root: u32, to: &mut impl Pool, at: u32, order: &mut [u32]) -> u32 {
    let (first, z0) = root_slot(root);
    order[0] = first / overwrite::CHILDREN;
    let (mut i, mut n) = (0, 1);
    while i < n {
        let (src, dst) = (
            overwrite::CHILDREN * order[i],
            overwrite::CHILDREN * (at + i as u32),
        );
        let mut k = 0;
        while k < overwrite::CHILDREN {
            let under = under(from, src + k);
            let moved = if under == NONE {
                0
            } else {
                order[n] = under / overwrite::CHILDREN;
                n += 1;
                BRANCH | (overwrite::CHILDREN * (at + n as u32 - 1))
            };
            to.set_first_child(dst + k, moved);
            to.set_data(dst + k, from.data(src + k));
            k += 1;
        }
        i += 1;
    }
    root_word(at, z0)
}

/// How a reader reads a leaf's value: the ground's void for a bore's air,
/// as the ways write it (`way::AIR`), else itself; `ground::VOID` where
/// the tree holds nothing.
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
    fn past_the_bricks_most() {
        assert!(past(1, BRICK_CHILD_GROUPS));
    }
}
