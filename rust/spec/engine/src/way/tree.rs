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
//! pool every brick's tree shares, off a free stack, and a brick's root
//! word names the child group of its nodes along z (`root_word`,
//! `clipmap::ROOT_AT`), from which a reader steps down as the placed tree's
//! does (`cursor`).
//!
//! Built a level at a time, one brick to a workgroup (pxl-shader
//! `way_tree_cs`), top-down then back up:
//!
//! - the brick's nodes along z its ways' bound meets are the first
//!   level's candidates, a slot each of its root child group (`span`, `root`);
//! - each candidate coarser than the cells is flagged where its z meets
//!   the ways' bound in the brick's levels (`flag`): a flagged one takes a
//!   child group for its children,
//!   the next level's candidates (`branch`), the rest hold nothing
//!   (`empty`); a level's child groups come off the stack together, `fits` the
//!   check before;
//! - the candidates at the cells are leaves (`leaf`);
//! - from the deepest level up, each branch is folded, and one whose eight
//!   are one leaf, nothing included, is written as that leaf and its child group
//!   given back (`fold`): the tree held is the one the rule defines, its
//!   child groups the fewest that hold it.
//!
//! A candidate's node is held in its data word while the tree is built,
//! where its leaf or fold is written after (`pack`).

use crate::clipmap;
use crate::clipmap::{Bricks, BRICK_SHIFT};
use crate::fill;
use crate::overwrite::{self, Columns, NodeSlot, AIR as PLACED_AIR, BRANCH};
use crate::{ground, Zoom, LEAF_CELL};

/// The tree's columns as its builder writes them, a word a slot.
pub trait Pool: Columns {
    fn set_first_child(&mut self, slot: u32, v: u32);
    fn set_data(&mut self, slot: u32, v: u32);
}

/// A pool in one column, its child pointers, then its data words: the GPU's
/// buffer (pxl-render gpu/ray/ground.rs) and a bake's.
pub struct Column<'a>(pub &'a mut [u32]);

impl Columns for Column<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, self.0.len() as u32 / 2 + slot)
    }
}

impl Pool for Column<'_> {
    fn set_first_child(&mut self, slot: u32, v: u32) {
        fill::put(self.0, slot, v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        let half = self.0.len() as u32 / 2;
        fill::put(self.0, half + slot, v);
    }
}

/// A pool in one column as a reader holds it, its child pointers, then its data
/// words: the cast's binding, and the CPU's mirror of the GPU's.
#[derive(Clone, Copy)]
pub struct Read<'a>(pub &'a [u32]);

impl Columns for Read<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        crate::at(self.0, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, self.0.len() as u32 / 2 + slot)
    }
}

/// The most child groups the pool grows to, eight slots each, every brick's
/// tree's: a bake's set fits it wherever the eye is (pxl-spec
/// bake/tile/terrain.rs `pool_needs`), so a pool this large refuses none.
/// The pool starts smaller and grows as a build runs out, up to this
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::grow`), as the CPU's
/// mirror does (pxl-kernel domain/layer/terrain/mirror.rs); a build past
/// what it holds is refused (`fits`), its brick left absent. Child group 0
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
const ROOT_ID_BITS: u32 = POOL_CHILD_GROUPS.trailing_zeros();
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

/// A handed brick's scratch past its spans (`clipmap::TREE_AT`), a frame's,
/// for a tree of `child_groups` at most: the child groups its build gave back, their
/// count first, for the pushes after it (pxl-shader `way_return_cs`); then
/// the child groups its tree holds from `records_at`, their count first, a
/// record each, its child group's id and its eight child pointers and eight data words,
/// which the read-back carries to the CPU's mirror. A build takes no more
/// than its scratch holds (`is_cramped`), so neither runs past its room. The
/// renderer starts it small and grows it (pxl-render gpu/ray/ground.rs
/// `GroundBuffers::grow_scratch`), every brick's alike, so a frame's
/// scratch is `clipmap::FILL_CAP` of them and each brick's length is a
/// `FILL_CAP`th of its buffer's (pxl-engine fill.rs `scratch_at`); at
/// `BRICK_CHILD_GROUPS` it is `SCRATCH_WORDS`.
pub const FREED_AT: u32 = clipmap::TREE_AT;
pub const RECORD_WORDS: u32 = 1 + CHILD_GROUP_WORDS;

/// A brick's scratch for a tree of `child_groups` at most, in words.
pub const fn scratch_words(child_groups: u32) -> u32 {
    FREED_AT + 2 + child_groups * (1 + RECORD_WORDS)
}

/// The child groups a brick's scratch of `words` holds a tree of, no more than
/// `BRICK_CHILD_GROUPS`: `scratch_words`'s inverse, none where it holds none.
pub const fn scratch_child_groups(words: u32) -> u32 {
    if words < FREED_AT + 2 {
        return 0;
    }
    let child_groups = (words - FREED_AT - 2) / (1 + RECORD_WORDS);
    if child_groups < BRICK_CHILD_GROUPS {
        child_groups
    } else {
        BRICK_CHILD_GROUPS
    }
}

/// Where a brick's records begin in its scratch for a tree of `child_groups`.
pub const fn records_at(child_groups: u32) -> u32 {
    FREED_AT + 1 + child_groups
}

/// A brick's scratch at its most, a tree of `BRICK_CHILD_GROUPS`: what a bake
/// or a test fills one brick in.
pub const SCRATCH_WORDS: u32 = scratch_words(BRICK_CHILD_GROUPS);

/// A build's records' count where its tree would take more child groups than its
/// scratch holds (`is_cramped`), short of the brick's own most: no record
/// kept and its root word 0, which the renderer reads as the scratch's to
/// grow; `STARVED` the pool's, and `NONE` the brick's own most (pxl-render
/// gpu/ray/ground.rs `Short`). Each past any count of a brick's, so a count
/// of none is a brick built with no ways, its root word 0.
pub const CRAMPED: u32 = NONE - 1;
pub const STARVED: u32 = NONE - 2;
const _: () = assert!(
    STARVED > BRICK_CHILD_GROUPS,
    "no count of records is a mark"
);

/// A candidate's word: its X and Y in the brick at its zoom,
/// `BRICK_SHIFT` bits each, and its Z.
const REL_MASK: u32 = (1 << BRICK_SHIFT) - 1;
const Z_SHIFT: u32 = 2 * BRICK_SHIFT;
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
/// brick's levels at its size (`clipmap::Bricks::level`, its slot `s`).
pub fn flag(pool: &impl Pool, w: &impl Bricks, s: u32, cell: Zoom, zoom: Zoom, slot: u32) -> bool {
    let at = unpack(pool.data(slot));
    if zoom >= cell {
        return false;
    }
    let e = [at[0] as u32, at[1] as u32];
    let [lo, hi] = ground::bounds_of(w.level(cell, s, cell - zoom, e))[1];
    let d = LEAF_CELL - zoom;
    lo < (at[2] + 1) << d && hi > at[2] << d
}

/// Whether a level's child groups, a flagged candidate's each, may be taken
/// together: the pool has `flagged` left of `left`, and the brick, having
/// taken `taken`, stays within `BRICK_CHILD_GROUPS`. None is taken past either:
/// the tree is refused, its root word 0 and its records' count `STARVED`,
/// which the renderer reads as the pool's to grow or, at its cap, the
/// brick's to stay absent; its count `NONE` where it is `past` the brick's most,
/// absent at once (pxl-render gpu/ray/ground.rs `Resident`). The bake bounds
/// a set's and a brick's (pxl-geo-bake terrain/pool.rs).
pub fn fits(left: u32, taken: u32, flagged: u32) -> bool {
    flagged <= left && !past(taken, flagged)
}

/// Whether a level's `flagged` child groups take a brick that has taken `taken`
/// past its most, `BRICK_CHILD_GROUPS`: refused whatever the pool holds, so its
/// build keeps `NONE` for its records' count rather than `STARVED`, which
/// the pool's growth answers (pxl-shader `way_tree_cs`).
pub fn past(taken: u32, flagged: u32) -> bool {
    flagged > BRICK_CHILD_GROUPS - taken
}

/// Whether a level's `flagged` child groups take a brick that has taken `taken`
/// past the `child_groups` its scratch holds (`scratch_child_groups`), whose freed
/// child groups and records each have room for so many: not taken, its build
/// keeping `CRAMPED` for its records' count, which the scratch's growth
/// answers (pxl-shader `way_tree_cs`). Asked after `past`, so it marks
/// only what a larger scratch holds.
pub fn is_cramped(child_groups: u32, taken: u32, flagged: u32) -> bool {
    taken + flagged > child_groups
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
/// leaf: a brick's old tree walked, its child groups given back before it is
/// built again.
pub fn under(pool: &impl Pool, slot: u32) -> u32 {
    let h = pool.first_child(slot);
    if h == 0 {
        NONE
    } else {
        h & !BRANCH
    }
}

/// A brick's records at their most as the read-back carries them: its
/// scratch from `records_at` on, their count, then a child group its tree holds
/// each.
pub(crate) const RECORDS_WORDS: u32 = 1 + BRICK_CHILD_GROUPS * RECORD_WORDS;
const _: () = assert!(
    RECORDS_WORDS == SCRATCH_WORDS - records_at(BRICK_CHILD_GROUPS),
    "the records end the scratch"
);

/// A brick's records in bytes at their most: the most a read-back
/// copies off its scratch, which copies what its room holds
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::record_bytes`).
pub const RECORDS_BYTES: u32 = RECORDS_WORDS * 4;

/// A brick's records (`RECORDS_WORDS`) laid into `pool` at the slots they
/// came from, each its child group's id and its eight child pointers and eight data words:
/// the CPU's mirror of the GPU's pool, where they land, so a cursor reads
/// one tree on both sides.
pub fn lay(pool: &mut impl Pool, records: &[u32]) {
    let n = crate::at(records, 0);
    let mut r = 0;
    while r < n && r < BRICK_CHILD_GROUPS {
        let at = 1 + r * RECORD_WORDS;
        let first = overwrite::CHILDREN * crate::at(records, at);
        let mut k = 0;
        while k < overwrite::CHILDREN {
            pool.set_first_child(first + k, crate::at(records, at + 1 + k));
            pool.set_data(
                first + k,
                crate::at(records, at + 1 + overwrite::CHILDREN + k),
            );
            k += 1;
        }
        r += 1;
    }
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
    use crate::clipmap;

    #[test]
    fn scratch_holds_child_groups() {
        // At a brick's most, the layout it had before it grew: its spans,
        // its freed child groups' count and ids, its records' count and records.
        assert_eq!(
            SCRATCH_WORDS,
            clipmap::TREE_AT + 1 + BRICK_CHILD_GROUPS + 1 + BRICK_CHILD_GROUPS * RECORD_WORDS
        );
        for child_groups in [1, 539, 1078, BRICK_CHILD_GROUPS] {
            assert_eq!(
                scratch_child_groups(scratch_words(child_groups)),
                child_groups
            );
            // A word short holds a child group fewer.
            assert_eq!(
                scratch_child_groups(scratch_words(child_groups) - 1),
                child_groups - 1
            );
            // Its records end it.
            assert_eq!(
                records_at(child_groups) + 1 + child_groups * RECORD_WORDS,
                scratch_words(child_groups)
            );
        }
        assert_eq!(
            scratch_child_groups(scratch_words(BRICK_CHILD_GROUPS) + 1000),
            BRICK_CHILD_GROUPS
        );
        assert_eq!(scratch_child_groups(FREED_AT + 1), 0);
    }

    #[test]
    fn cramped_past_scratch() {
        assert!(!is_cramped(8, 1, 7));
        assert!(is_cramped(8, 1, 8));
        // Past the brick's most is `past`'s, asked first.
        assert!(past(1, BRICK_CHILD_GROUPS));
    }
}
