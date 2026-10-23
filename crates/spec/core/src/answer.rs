// pxl-spec-core/answer.rs

//! gpu-portable: what the world's one query answers of a node
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The query between them"), as every source writes it and every reader
//! reads it: its kind, a block's value in the one value space
//! (doc/minutes/20261005-the-ground-is-a-carto-palette.md), and a split's
//! slab along z. Below the descent, the query and the sources alike.

use crate::cube::{object, sentinel};

/// What a node is to a descent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Coarser than asked: something under it may fill.
    Split,
    /// A block: at the zoom asked, what it is; coarser, air (`AIR`) where
    /// nothing under it fills at the zooms asked, as a block of air its size.
    Block,
    /// Not held: past it the ray tells nothing.
    Lacks,
}

/// What is in a node: its kind, and the block where it is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct In<B> {
    pub kind: Kind,
    pub block: B,
}

/// What no block fills: void, no looks.
pub const AIR: [u32; 2] = [sentinel::void as u32, 0];

/// A block's word where what is placed answered it, over its value: the
/// source control tells an edit by (`placed`), which nothing drawn reads.
pub const PLACED_BLOCK: u32 = 1 << 16;

/// A bound as a word, its lowest in the low half and its highest in the
/// high, each an i16: a level's entry (`ground::Bounds`) and a split's slab
/// (`slab_word`) alike.
pub fn bound_word([lo, hi]: [i32; 2]) -> i32 {
    lo & 0xffff | hi << 16
}

/// `bound_word`'s lowest and highest.
pub fn bound_of(w: i32) -> [i32; 2] {
    [w << 16 >> 16, w >> 16]
}

/// A split's block word: where anything may fill the node along z, in leaf
/// cells, `[lo, hi)` held as its lowest in the low half and its highest,
/// `hi − 1`, in the high (`bound_word`), each held to i16's range — the
/// world's whole z, so i16's whole is anything along it (`ANY`), and
/// lowest over highest is nothing (`ground::EMPTY`), as a join keeps it. A
/// ray through the node outside it meets nothing there (pxl-spec-core cast).
/// Each source's own bound joined: the ground's (`ground::slab`), the
/// ways' (`ways::slab`), and what is placed (`overwrite::slab`), each off
/// what it holds, as each source answers.
pub fn slab_word([lo, hi]: [i32; 2]) -> u32 {
    let held = |v: i32| {
        if v < i16::MIN as i32 {
            i16::MIN as i32
        } else if v > i16::MAX as i32 {
            i16::MAX as i32
        } else {
            v
        }
    };
    bound_word([held(lo), held(hi - 1)]) as u32
}

/// `slab_word`'s lowest and past its highest.
pub fn slab_of(w: u32) -> [i32; 2] {
    let [lo, last] = bound_of(w as i32);
    [lo, last + 1]
}

/// Anything along z the world holds, as a slab: from its lowest leaf cell to
/// past its highest, half-open, i16's whole as `slab_word` holds it. Not a
/// relief of face heights, both held (`ground::UNBOUNDED`), which is another
/// fact in another unit.
pub const ANY: [i32; 2] = [i16::MIN as i32, i16::MAX as i32 + 1];
const _: () = assert!(crate::LEAF_CELL == crate::UNIT_ZOOM, "a slab's leaf cell is the world's unit");
const _: () = assert!(
    -(1i32 << (crate::HEIGHT_BITS - 1)) == i16::MIN as i32 && (1i32 << (crate::HEIGHT_BITS - 1)) - 1 == i16::MAX as i32,
    "the world's z, in leaf cells, is i16's range"
);

/// Whether a block's value fills its block: neither void nor air, whichever
/// source holds it. What is placed answers a voxel alone
/// (`overwrite::value`), so no decor or marker is among them.
pub fn fills(value: u32) -> bool {
    let v = value & !PLACED_BLOCK;
    v != sentinel::void as u32 && v != object::air as u32
}

/// Whether what is placed holds a block's value (`PLACED_BLOCK`).
pub fn placed(value: u32) -> bool {
    value & PLACED_BLOCK != 0
}
