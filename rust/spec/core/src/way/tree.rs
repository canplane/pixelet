// pxl-spec-core/way/tree.rs

//! gpu-portable: a square's ways as blocks, in a tree of the placed tree's
//! shape (pxl-spec-core overwrite.rs), so a reader asks them by the one
//! rule it asks what is placed by
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): from the
//! square's own nodes along z down to its cells, a node's eight slots a
//! family, children only where they differ, each branch its eight folded
//! (`overwrite::fold`), its leaves what the ways write at its cells
//! (`way::lined`, read off the square's spans). A reader asks a square's
//! ways only of a node the square's or under it, a wider one the windows'
//! bound over its squares (`window::bound_over`), so the tree holds nothing
//! over the square. Its families are taken from one pool every square's
//! tree shares, off a free stack, and a square's root word names the family
//! of its nodes along z (`root_word`, `window::ROOT_AT`), from which a
//! reader steps down as the placed tree's does (`cursor`).
//!
//! Built a level at a time, one square to a workgroup (pxl-shader
//! `way_tree_cs`), top-down then back up:
//!
//! - the square's nodes along z its ways' bound meets are the first
//!   level's candidates, a slot each of its root family (`span`, `root`);
//! - each candidate coarser than the cells is flagged where its z meets
//!   the ways' bound in the square's levels (`flag`): a flagged one takes a
//!   family for its children,
//!   the next level's candidates (`branch`), the rest hold nothing
//!   (`empty`); a level's families come off the stack together, `fits` the
//!   check before;
//! - the candidates at the cells are leaves (`leaf`);
//! - from the deepest level up, each branch is folded, and one whose eight
//!   are one leaf, nothing included, is written as that leaf and its family
//!   given back (`fold`): the tree held is the one the rule defines, its
//!   families the fewest that hold it.
//!
//! A candidate's node is held in its data word while the tree is built,
//! where its leaf or fold is written after (`pack`).

use crate::overwrite::{self, Held, Lanes, AIR as PLACED_AIR, BRANCH};
use crate::window::{Squares, SQUARE_SHIFT};
use crate::{ground, Zoom, LEAF_CELL};

/// The tree's lanes as its builder writes them, a word a slot.
pub trait Pool: Lanes {
    fn set_heap(&mut self, slot: u32, v: u32);
    fn set_data(&mut self, slot: u32, v: u32);
}

/// A pool in one lane, its heap words, then its data words: the GPU's
/// buffer (pxl-render gpu/ray/ground.rs) and a bake's.
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

/// The most families the pool grows to, eight slots each, every square's
/// tree's: a bake's set fits it wherever the eye is (pxl-spec
/// bake/tile/terrain.rs `pool_needs`), so a pool this large refuses none.
/// The pool starts smaller and grows as a build runs out
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::grow`), up to the kernel's
/// cap (pxl-kernel domain/layer/terrain/param.rs `WAYS_POOL_BYTES`), no more than this; a build
/// past what it holds is refused (`fits`), its square left absent. Family 0
/// is never handed out: its words stay zero, nothing (`overwrite::Lanes`),
/// so a root word 0 is a square with no tree, read as nothing without a
/// family of its own (`root_word`).
pub const POOL_FAMILIES: u32 = 1 << 20;

/// A square's root word (`window::ROOT_AT`): its root family's id, under
/// `POOL_FAMILIES`, and over it the Z of the node its first slot holds, in
/// the bits left; 0, family 0, where the square has no tree.
pub const fn root_word(family: u32, z0: i32) -> u32 {
    family | (z0 as u32) << ROOT_ID_BITS
}

/// A root word's family's first slot and its first slot's Z.
pub const fn root_slot(w: u32) -> (u32, i32) {
    (8 * (w & (POOL_FAMILIES - 1)), w as i32 >> ROOT_ID_BITS)
}

/// A root word's family id bits, `POOL_FAMILIES`'s.
const ROOT_ID_BITS: u32 = POOL_FAMILIES.trailing_zeros();
const _: () = assert!(
    POOL_FAMILIES.is_power_of_two()
        && (i16::MIN as i32 >> SQUARE_SHIFT) >= -(1 << (31 - ROOT_ID_BITS)),
    "a root word holds every family's id and a square's Z, the leaves' i16 over its side"
);

/// The most of a square's own nodes along z its root family holds, a slot
/// each: a square is a node `SQUARE_SHIFT` zooms over its cells, and its
/// ways' bound meets no more of them than this; a tile past it fails its
/// bake (pxl-spec bake/tile/terrain.rs `fill_fit`), its build refused as one
/// `past` its families. The most of the staged z15 set is 4 (2026-10-27).
pub const ROOT_NODES: u32 = overwrite::FAMILY;

/// The square's own nodes along z its ways' bound `[lo, hi)` (leaf cells,
/// `ground::Bounds`) meets, its cells at `cell`: the first's Z and how
/// many, none where the bound is empty.
pub fn span([lo, hi]: [i32; 2], cell: Zoom) -> (i32, u32) {
    let d = LEAF_CELL - (cell - SQUARE_SHIFT);
    if lo >= hi {
        (0, 0)
    } else {
        (lo >> d, (((hi - 1) >> d) - (lo >> d) + 1) as u32)
    }
}

/// What the node `at` of `zoom` holds of the square's ways' tree whose root
/// word is `root`, its cells at `cell`, the node the square's or under it:
/// the square's own node along z over it in the root family, then down
/// (`overwrite::down`), as `overwrite::cursor` steps; nothing where the root
/// holds no node there. A descent asks this once, where it finds the
/// square (pxl-spec-core world.rs `node`).
pub fn cursor(lanes: &impl Lanes, root: u32, cell: Zoom, zoom: Zoom, at: [i32; 3]) -> Held {
    let (first, z0) = root_slot(root);
    let top = cell - SQUARE_SHIFT;
    if root == 0 || zoom < top {
        return Held::NOTHING;
    }
    let k = (at[2] >> (zoom - top)) - z0;
    if k < 0 || k >= ROOT_NODES as i32 {
        return Held::NOTHING;
    }
    overwrite::down(lanes, first + k as u32, top, zoom, at)
}

/// A family's words in a pool: its eight heap words and its eight data
/// words.
pub const FAMILY_WORDS: u32 = 16;

/// The most families one square's build takes before its fold gives any
/// back: the most of the format-12 set (2026-10-18, its windows' squares
/// lined as before the face block, which only lines more); a tile past it
/// fails its bake (pxl-geo-bake terrain/pool.rs), and what a build keeps a
/// square, its records and the families its fold gives back, is held to it.
pub const SQUARE_FAMILIES: u32 = 24_067;

/// No slot: a fold that gives no family back.
pub const NONE: u32 = u32::MAX;

/// A handed square's scratch past its spans (`window::TREE_AT`), a frame's,
/// for a tree of `families` at most: the families its build gave back, their
/// count first, for the pushes after it (pxl-shader `way_freed_cs`); then
/// the families its tree holds from `records_at`, their count first, a
/// record each, its family's id and its eight heap and eight data words,
/// which the read-back carries to the CPU's mirror. A build takes no more
/// than its scratch holds (`cramped`), so neither runs past its room. The
/// renderer starts it small and grows it (pxl-render gpu/ray/ground.rs
/// `GroundBuffers::grow_scratch`), every square's alike, so a frame's
/// scratch is `window::HANDED` of them and each square's length is a
/// `HANDED`th of its buffer's (pxl-spec-core fill.rs `scratch_at`); at
/// `SQUARE_FAMILIES` it is `SCRATCH_WORDS`.
pub const FREED_AT: u32 = crate::window::TREE_AT;
pub const RECORD_WORDS: u32 = 17;

/// A square's scratch for a tree of `families` at most, in words.
pub const fn scratch_words(families: u32) -> u32 {
    FREED_AT + 2 + families * (1 + RECORD_WORDS)
}

/// The families a square's scratch of `words` holds a tree of, no more than
/// `SQUARE_FAMILIES`: `scratch_words`'s inverse, none where it holds none.
pub const fn scratch_families(words: u32) -> u32 {
    if words < FREED_AT + 2 {
        return 0;
    }
    let families = (words - FREED_AT - 2) / (1 + RECORD_WORDS);
    if families < SQUARE_FAMILIES {
        families
    } else {
        SQUARE_FAMILIES
    }
}

/// Where a square's records begin in its scratch for a tree of `families`.
pub const fn records_at(families: u32) -> u32 {
    FREED_AT + 1 + families
}

/// A square's scratch at its most, a tree of `SQUARE_FAMILIES`: what a bake
/// or a test fills one square in.
pub const SCRATCH_WORDS: u32 = scratch_words(SQUARE_FAMILIES);

/// A build's records' count where its tree would take more families than its
/// scratch holds (`cramped`), short of the square's own most: no record
/// kept and its root word 0, which the renderer reads as the scratch's to
/// grow; `STARVED` the pool's, and `NONE` the square's own most (pxl-render
/// gpu/ray/ground.rs `Short`). Each past any count of a square's, so a count
/// of none is a square built with no ways, its root word 0.
pub const CRAMPED: u32 = NONE - 1;
pub const STARVED: u32 = NONE - 2;
const _: () = assert!(STARVED > SQUARE_FAMILIES, "no count of records is a mark");

/// A candidate's word: its X and Y in the square at its zoom,
/// `SQUARE_SHIFT` bits each, and its Z.
const REL_MASK: u32 = (1 << SQUARE_SHIFT) - 1;
const Z_SHIFT: u32 = 2 * SQUARE_SHIFT;
const _: () = assert!(
    32 - Z_SHIFT > 16,
    "a cell's Z, an i16 at the leaves, in a candidate's word"
);

/// The candidate word of the node `at`, its X and Y in the square.
fn pack(at: [i32; 3]) -> u32 {
    (at[2] as u32) << Z_SHIFT | (at[1] as u32) << SQUARE_SHIFT | at[0] as u32
}

/// A candidate word's node, its X and Y in the square.
fn unpack(w: u32) -> [i32; 3] {
    [
        (w & REL_MASK) as i32,
        ((w >> SQUARE_SHIFT) & REL_MASK) as i32,
        (w as i32) >> Z_SHIFT,
    ]
}

/// The first level: the square's own nodes along z from `z0`, in the family
/// at `root` as `cursor` reads them, each a candidate.
pub fn root(pool: &mut impl Pool, z0: i32, root: u32) {
    let mut k = 0;
    while k < ROOT_NODES {
        pool.set_heap(root + k, 0);
        pool.set_data(root + k, pack([0, 0, z0 + k as i32]));
        k += 1;
    }
}

/// Whether the candidate at `slot`, a node of `zoom`, takes a family for
/// its children: it is coarser than the square's cells at `cell`, and its
/// Z meets where the square's ways write under it, the ways' bound of the
/// square's levels at its size (`window::Squares::level`, its slot `s`).
pub fn flag(pool: &impl Pool, w: &impl Squares, s: u32, cell: Zoom, zoom: Zoom, slot: u32) -> bool {
    let at = unpack(pool.data(slot));
    if zoom >= cell {
        return false;
    }
    let e = [at[0] as u32, at[1] as u32];
    let [lo, hi] = ground::bounds_of(w.level(cell, s, cell - zoom, e))[1];
    let d = LEAF_CELL - zoom;
    lo < (at[2] + 1) << d && hi > at[2] << d
}

/// Whether a level's families, a flagged candidate's each, may be taken
/// together: the pool has `flagged` left of `left`, and the square, having
/// taken `taken`, stays within `SQUARE_FAMILIES`. None is taken past either:
/// the tree is refused, its root word 0 and its records' count `STARVED`,
/// which the renderer reads as the pool's to grow or, at its cap, the
/// square's to stay absent; its count `NONE` where it is `past` the square's most,
/// absent at once (pxl-render gpu/renderer.rs `Landing`). The bake bounds
/// a set's and a square's (pxl-geo-bake terrain/pool.rs).
pub fn fits(left: u32, taken: u32, flagged: u32) -> bool {
    flagged <= left && !past(taken, flagged)
}

/// Whether a level's `flagged` families take a square that has taken `taken`
/// past its most, `SQUARE_FAMILIES`: refused whatever the pool holds, so its
/// build keeps `NONE` for its records' count rather than `STARVED`, which
/// the pool's growth answers (pxl-shader `way_tree_cs`).
pub fn past(taken: u32, flagged: u32) -> bool {
    flagged > SQUARE_FAMILIES - taken
}

/// Whether a level's `flagged` families take a square that has taken `taken`
/// past the `families` its scratch holds (`scratch_families`), whose freed
/// families and records each have room for so many: not taken, its build
/// keeping `CRAMPED` for its records' count, which the scratch's growth
/// answers (pxl-shader `way_tree_cs`). Asked after `past`, so it marks
/// only what a larger scratch holds.
pub fn cramped(families: u32, taken: u32, flagged: u32) -> bool {
    taken + flagged > families
}

/// The flagged candidate at `slot`, a branch whose children's family begins
/// at `first`: its eight, the next level's candidates.
pub fn branch(pool: &mut impl Pool, slot: u32, first: u32) {
    let at = unpack(pool.data(slot));
    pool.set_heap(slot, BRANCH | first);
    let mut k = 0;
    while k < 8 {
        let child = [
            2 * at[0] + (k & 1) as i32,
            2 * at[1] + (k >> 1 & 1) as i32,
            2 * at[2] + (k >> 2) as i32,
        ];
        pool.set_heap(first + k, 0);
        pool.set_data(first + k, pack(child));
        k += 1;
    }
}

/// A candidate not flagged: nothing under it.
pub fn empty(pool: &mut impl Pool, slot: u32) {
    pool.set_heap(slot, 0);
    pool.set_data(slot, ground::VOID);
}

/// The candidate at `slot`, a cell of the square: a leaf of what the ways
/// write there (`way::lined`, off the spans its fill kept), nothing where
/// they write none. A bore's air is placed air in the tree, filling what it
/// holds (`overwrite::voxel`), and the ground's void where a reader reads
/// it, as the ways write it (`way::AIR`).
pub fn leaf(pool: &mut impl Pool, kept: &impl super::Kept, cell: Zoom, slot: u32) {
    let at = unpack(pool.data(slot));
    let value = super::lined(kept, [at[0], at[1]], cell, at[2]);
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
/// slot of the family it gives back; else its eight folded by volume
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

/// The first slot of the family under the branch at `slot`, `NONE` under a
/// leaf: a square's old tree walked, its families given back before it is
/// built again.
pub fn under(pool: &impl Pool, slot: u32) -> u32 {
    let h = pool.heap(slot);
    if h == 0 {
        NONE
    } else {
        h & !BRANCH
    }
}

/// A square's records at their most as the read-back carries them: its
/// scratch from `records_at` on, their count, then a family its tree holds
/// each.
pub const RECORDS_WORDS: u32 = 1 + SQUARE_FAMILIES * RECORD_WORDS;
const _: () = assert!(
    RECORDS_WORDS == SCRATCH_WORDS - records_at(SQUARE_FAMILIES),
    "the records end the scratch"
);

/// A square's records in bytes at their most: the most a read-back
/// copies off its scratch, which copies what its room holds
/// (pxl-render gpu/ray/ground.rs `GroundBuffers::record_bytes`).
pub const RECORDS_BYTES: u32 = RECORDS_WORDS * 4;

/// A square's records (`RECORDS_WORDS`) laid into `pool` at the slots they
/// came from, each its family's id and its eight heap and eight data words:
/// the CPU's mirror of the GPU's pool, where they land, so a cursor reads
/// one tree on both sides.
pub fn lay(pool: &mut impl Pool, records: &[u32]) {
    let n = crate::at(records, 0);
    let mut r = 0;
    while r < n && r < SQUARE_FAMILIES {
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
/// as the ways write it (`way::AIR`), else itself; `ground::VOID` where
/// the tree holds nothing.
pub fn read(value: u32) -> u32 {
    if value == PLACED_AIR {
        super::AIR
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_holds_the_families_it_is_made_for() {
        // At a square's most, the layout it had before it grew: its spans,
        // its freed families' count and ids, its records' count and records.
        assert_eq!(
            SCRATCH_WORDS,
            crate::window::TREE_AT + 1 + SQUARE_FAMILIES + 1 + SQUARE_FAMILIES * RECORD_WORDS
        );
        for families in [1, 539, 1078, SQUARE_FAMILIES] {
            assert_eq!(scratch_families(scratch_words(families)), families);
            // A word short holds a family fewer.
            assert_eq!(scratch_families(scratch_words(families) - 1), families - 1);
            // Its records end it.
            assert_eq!(
                records_at(families) + 1 + families * RECORD_WORDS,
                scratch_words(families)
            );
        }
        assert_eq!(
            scratch_families(scratch_words(SQUARE_FAMILIES) + 1000),
            SQUARE_FAMILIES
        );
        assert_eq!(scratch_families(FREED_AT + 1), 0);
    }

    #[test]
    fn a_build_is_cramped_past_its_scratch_only() {
        assert!(!cramped(8, 1, 7));
        assert!(cramped(8, 1, 8));
        // Past the square's most is `past`'s, asked first.
        assert!(past(1, SQUARE_FAMILIES));
    }
}
