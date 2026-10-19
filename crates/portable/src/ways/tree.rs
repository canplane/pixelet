// pxl-portable/ways/tree.rs

//! gpu-portable: a square's ways as blocks, in a tree of the placed tree's
//! shape (pxl-portable overwrite.rs), so a reader asks them by the one
//! rule it asks what is placed by
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): from the
//! world's zoom-0 block down to the square's cells, eight slots a block,
//! children only where they differ, each branch its eight folded
//! (`overwrite::fold`), its leaves what the ways write at its cells
//! (`ways::lined`, read off the square's runs). Its blocks are taken from
//! one pool every square's tree shares, off a free stack, and a square's
//! root word names its zoom-0 block (`window::ROOT_AT`), read as the placed
//! tree's is (`overwrite::cursor`).
//!
//! Built a level at a time, one square to a workgroup (gpu-query-shader
//! `ways_tree_cs`), top-down then back up:
//!
//! - the zoom-0 block's eight are the first level's candidates (`root`);
//! - each candidate coarser than the cells is flagged where it is the
//!   square's or under it and its z meets the ways' bound in the square's
//!   levels (`flag`): a flagged one takes a block of eight for its children,
//!   the next level's candidates (`branch`), the rest hold nothing
//!   (`empty`); a level's blocks come off the stack together, `fits` the
//!   check before;
//! - the candidates at the cells are leaves (`leaf`);
//! - from the deepest level up, each branch is folded, and one whose eight
//!   are one leaf, nothing included, is written as that leaf and its block
//!   given back (`fold`): the tree held is the one the rule defines, its
//!   blocks the fewest that hold it.
//!
//! A candidate's node is held in its data word while the tree is built,
//! where its leaf or fold is written after (`pack`).

use crate::overwrite::{self, Lanes, AIR as PLACED_AIR, BRANCH};
use crate::window::{Squares, LEVELS, SQUARE_SHIFT};
use crate::{ground, Zoom, LEAF_CELL};

/// The tree's lanes as its builder writes them, a word a slot.
pub trait Pool: Lanes {
    fn set_heap(&mut self, slot: u32, v: u32);
    fn set_data(&mut self, slot: u32, v: u32);
}

/// A pool in one lane, its heap words, then its data words: the GPU's
/// buffer (pxl-render gpu/renderer.rs) and a bake's.
pub struct Lane<'a>(pub &'a mut [u32]);

impl Lanes for Lane<'_> {
    fn heap(&self, slot: u32) -> u32 {
        crate::at(self.0, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, self.0.len() as u32 / 2 + slot)
    }
}

impl Pool for Lane<'_> {
    fn set_heap(&mut self, slot: u32, v: u32) {
        crate::fill::put(self.0, slot, v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        let half = self.0.len() as u32 / 2;
        crate::fill::put(self.0, half + slot, v);
    }
}

/// A pool in one lane as a reader holds it, its heap words, then its data
/// words: the cast's binding, and the CPU's mirror of the GPU's.
#[derive(Clone, Copy)]
pub struct Read<'a>(pub &'a [u32]);

impl Lanes for Read<'_> {
    fn heap(&self, slot: u32) -> u32 {
        crate::at(self.0, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        crate::at(self.0, self.0.len() as u32 / 2 + slot)
    }
}

/// A square's runs as its fill wrote them (`ways::runs`): column `i`'s
/// first run and the next column's, and run `r`.
pub trait Runs {
    fn column(&self, i: u32) -> [u32; 2];
    fn run(&self, r: u32) -> u32;
}

/// The square a tree is of: its cells' zoom, and its index among the squares
/// at it, X counted from the round's start (`window::square_of`).
#[derive(Clone, Copy)]
pub struct Of {
    pub cell: Zoom,
    pub at: [i32; 2],
}

impl Of {
    /// The zoom a square is a node at: a square's cells, `SQUARE_SHIFT`
    /// zooms coarser.
    const fn zoom(self) -> Zoom {
        self.cell - SQUARE_SHIFT
    }
}

/// The pool's blocks, eight slots each, every square's tree's: a bake's set
/// fits it wherever the eye is (pxl-spec bake/tile/terrain.rs
/// `pool_needs`), so the free stack runs out of none, and a build past it
/// fails loudly (`fits`). Block 0 is never handed out: its words stay
/// zero, nothing (`overwrite::Lanes`), so a root word 0 is a square with
/// no tree, read as nothing without a word of its own (`window::ROOT_AT`).
pub const POOL_BLOCKS: u32 = 1 << 20;

/// The most blocks one square's build takes before its fold gives any
/// back: the most of the format-12 set (2026-10-18, its windows' squares
/// lined as before the face block, which only lines more); a tile past it
/// fails its bake (pxl-geo-bake terrain/pool.rs), and what a build keeps a
/// square, its records and the blocks its fold gives back, is held to it.
pub const SQUARE_BLOCKS: u32 = 24_067;

/// No slot: a fold that gives no block back.
pub const NONE: u32 = u32::MAX;

/// A handed square's scratch past its build, a frame's: the blocks its
/// build gave back, their count first, for the pushes after it
/// (gpu-query-shader `ways_freed_cs`); then the blocks its tree holds,
/// their count first, a record each, its block's id and its eight heap and
/// eight data words, which the read-back carries to the CPU's mirror.
pub const FREED_AT: u32 = 0;
pub const RECORDS_AT: u32 = FREED_AT + 1 + SQUARE_BLOCKS;
pub const RECORD_WORDS: u32 = 17;
pub const SCRATCH_WORDS: u32 = RECORDS_AT + 1 + SQUARE_BLOCKS * RECORD_WORDS;

/// A candidate's word: its X and Y under the square, `SQUARE_SHIFT` bits
/// each, none coarser than the square; whether it is off the square, a
/// node beside the square's own at a zoom over it; and its Z.
const REL_MASK: u32 = (1 << SQUARE_SHIFT) - 1;
const OFF: u32 = 1 << (2 * SQUARE_SHIFT);
const Z_SHIFT: u32 = 2 * SQUARE_SHIFT + 1;
const _: () = assert!(32 - Z_SHIFT > 16, "a cell's Z, an i16 at the leaves, in a candidate's word");

/// The square's node at `zoom`, no finer than the square, X counted from
/// the round's start (`overwrite::path`).
fn over(of: Of, zoom: Zoom) -> [i32; 2] {
    let s = of.zoom() - zoom;
    [(of.at[0] >> s) & (1u32 << zoom).wrapping_sub(1) as i32, of.at[1] >> s]
}

/// The candidate word of the node `at` of `zoom` under the root.
fn pack(of: Of, zoom: Zoom, at: [i32; 3]) -> u32 {
    let z = (at[2] as u32) << Z_SHIFT;
    if zoom <= of.zoom() {
        let o = over(of, zoom);
        return if at[0] == o[0] && at[1] == o[1] { z } else { z | OFF };
    }
    let s = zoom - of.zoom();
    let rel = [at[0] - (of.at[0] << s), at[1] - (of.at[1] << s)];
    z | (rel[1] as u32) << SQUARE_SHIFT | rel[0] as u32
}

/// A candidate word's node at `zoom`, and whether it is the square's or
/// under it.
fn unpack(of: Of, zoom: Zoom, w: u32) -> ([i32; 3], bool) {
    let z = (w as i32) >> Z_SHIFT;
    if zoom <= of.zoom() {
        let o = over(of, zoom);
        return ([o[0], o[1], z], w & OFF == 0);
    }
    let s = zoom - of.zoom();
    let rel = [(w & REL_MASK) as i32, ((w >> SQUARE_SHIFT) & REL_MASK) as i32];
    ([(of.at[0] << s) + rel[0], (of.at[1] << s) + rel[1], z], true)
}

/// The first level: the world's zoom-0 block at `root`, its eight nodes as
/// `overwrite::cursor` lays them, each a candidate.
pub fn root(pool: &mut impl Pool, of: Of, root: u32) {
    let mut k = 0;
    while k < 8 {
        let at = [(k & 1) as i32, -((k >> 1 & 1) as i32), -((k >> 2) as i32)];
        pool.set_heap(root + k, 0);
        pool.set_data(root + k, pack(of, 0, at));
        k += 1;
    }
}

/// Whether the candidate at `slot`, a node of `zoom`, takes a block for
/// its children: it is coarser than the square's cells, the square's or
/// under it, and its Z meets where the square's ways write under it, the
/// ways' bound of the square's levels at its size, the square's whole over
/// it (`window::Squares::level`, its slot `s`).
pub fn flag(pool: &impl Pool, w: &impl Squares, s: u32, of: Of, zoom: Zoom, slot: u32) -> bool {
    let (at, under) = unpack(of, zoom, pool.data(slot));
    if zoom >= of.cell || !under {
        return false;
    }
    let (j, e) = if zoom <= of.zoom() {
        (LEVELS, [0, 0])
    } else {
        let s = zoom - of.zoom();
        (of.cell - zoom, [(at[0] - (of.at[0] << s)) as u32, (at[1] - (of.at[1] << s)) as u32])
    };
    let [lo, hi] = ground::bounds_of(w.level(of.cell, s, j, e))[1];
    let d = LEAF_CELL - zoom;
    lo < (at[2] + 1) << d && hi > at[2] << d
}

/// Whether a level's blocks, a flagged candidate's each, may be taken
/// together: the pool has `flagged` left of `left`, and the square, having
/// taken `taken`, stays within `SQUARE_BLOCKS`. None is taken past either:
/// the tree fails loudly, as the bake bounds a set's and a square's
/// (pxl-geo-bake terrain/pool.rs).
pub fn fits(left: u32, taken: u32, flagged: u32) -> bool {
    flagged <= left && flagged <= SQUARE_BLOCKS - taken
}

/// The flagged candidate at `slot`, a node of `zoom`, a branch whose
/// children's block begins at `first`: its eight, the next level's
/// candidates.
pub fn branch(pool: &mut impl Pool, of: Of, zoom: Zoom, slot: u32, first: u32) {
    let (at, _) = unpack(of, zoom, pool.data(slot));
    pool.set_heap(slot, BRANCH | first);
    let mut k = 0;
    while k < 8 {
        let child = [2 * at[0] + (k & 1) as i32, 2 * at[1] + (k >> 1 & 1) as i32, 2 * at[2] + (k >> 2) as i32];
        pool.set_heap(first + k, 0);
        pool.set_data(first + k, pack(of, zoom + 1, child));
        k += 1;
    }
}

/// A candidate not flagged: nothing under it.
pub fn empty(pool: &mut impl Pool, slot: u32) {
    pool.set_heap(slot, 0);
    pool.set_data(slot, ground::VOID);
}

/// The candidate at `slot`, a cell of the square: a leaf of what the ways
/// write there, its column's run holding its Z (`ways::lined`'s, as the
/// fill wrote them, lowest first), found by halving, nothing where none
/// does. A bore's air is placed air in
/// the tree, filling what it holds (`overwrite::voxel`), and the ground's
/// void where a reader reads it, as the ways write it (`ways::AIR`).
pub fn leaf(pool: &mut impl Pool, runs: &impl Runs, of: Of, slot: u32) {
    let (at, under) = unpack(of, of.cell, pool.data(slot));
    let mut value = super::NOTHING;
    if under {
        let s = SQUARE_SHIFT;
        let i = crate::window::index_of([(at[0] - (of.at[0] << s)) as u32, (at[1] - (of.at[1] << s)) as u32]);
        // The first run past Z: the one before it holds Z.
        let [first, to] = runs.column(i);
        let (mut lo, mut hi) = (first, to);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if super::run_of(runs.run(mid)).0 <= at[2] {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo > first {
            value = super::run_of(runs.run(lo - 1)).1;
        }
    }
    let word = if value == super::NOTHING {
        ground::VOID
    } else if value == super::AIR {
        overwrite::leaf_word(PLACED_AIR, 0)
    } else {
        overwrite::leaf_word(value, 0)
    };
    pool.set_heap(slot, 0);
    pool.set_data(slot, word);
}

/// The branch at `slot`, a node of `zoom`, its children written: where its
/// eight are leaves of one word, nothing included, that leaf, and the first
/// slot of the block it gives back; else its eight folded by volume
/// (`overwrite::fold`, each child's share `overwrite::share`), and `NONE`.
pub fn fold(pool: &mut impl Pool, zoom: Zoom, slot: u32) -> u32 {
    let first = pool.heap(slot) & !BRANCH;
    let word = pool.data(first);
    let mut one = true;
    let mut k = 0;
    while k < 8 {
        one &= pool.heap(first + k) == 0 && pool.data(first + k) == word;
        k += 1;
    }
    if one {
        pool.set_heap(slot, 0);
        pool.set_data(slot, word);
        return first;
    }
    let share = |k: u32| {
        let d = pool.data(first + k);
        if pool.heap(first + k) == 0 {
            overwrite::share(d)
        } else {
            d
        }
    };
    let folded = overwrite::fold([share(0), share(1), share(2), share(3), share(4), share(5), share(6), share(7)], zoom);
    pool.set_data(slot, folded);
    NONE
}

/// The first slot of the block under the branch at `slot`, `NONE` under a
/// leaf: a square's old tree walked, its blocks given back before it is
/// built again.
pub fn under(pool: &impl Pool, slot: u32) -> u32 {
    let h = pool.heap(slot);
    if h == 0 {
        NONE
    } else {
        h & !BRANCH
    }
}

/// A square's records as the read-back carries them: its scratch from
/// `RECORDS_AT` on, their count, then a block its tree holds each.
pub const RECORDS_WORDS: u32 = SCRATCH_WORDS - RECORDS_AT;

/// A square's records in bytes: what a read-back copies off its scratch,
/// and how far a read-back's room lies after the one before
/// (`window::READS` of them).
pub const RECORDS_BYTES: u32 = RECORDS_WORDS * 4;

/// A square's records (`RECORDS_WORDS`) laid into `pool` at the slots they
/// came from, each its block's id and its eight heap and eight data words:
/// the CPU's mirror of the GPU's pool, where they land, so a cursor reads
/// one tree on both sides.
pub fn lay(pool: &mut impl Pool, records: &[u32]) {
    let n = crate::at(records, 0);
    let mut r = 0;
    while r < n && r < SQUARE_BLOCKS {
        let at = 1 + r * RECORD_WORDS;
        let first = 8 * crate::at(records, at);
        let mut k = 0;
        while k < 8 {
            pool.set_heap(first + k, crate::at(records, at + 1 + k));
            pool.set_data(first + k, crate::at(records, at + 9 + k));
            k += 1;
        }
        r += 1;
    }
}

/// How a reader reads a leaf's value: the ground's void for a bore's air,
/// as the ways write it (`ways::AIR`), else itself; `ground::VOID` where
/// the tree holds nothing.
pub fn read(value: u32) -> u32 {
    if value == PLACED_AIR {
        super::AIR
    } else {
        value
    }
}
