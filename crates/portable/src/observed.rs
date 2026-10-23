// pxl-portable/observed.rs

//! gpu-portable: the observed pyramid, as a GPU holds it
//! (cache/design-node-pool.md until it is a record): the nodes the eye's
//! rays asked about, each the world's one query's answer at the zooms it
//! was asked at and what was handed it after, so a node is asked once, not
//! once a ray. Present only where a ray asked; every node's parent present,
//! as rays reach a node through its parent. Nodes are kept in tiles of
//! eight siblings (Laine & Karras 2010; Crassin et al. 2009), under the
//! world's roots (`keeps`). What is drawn is the query's answer
//! whichever walk read it, so what is kept decides only how much is asked
//! again, never what is drawn.
//!
//! One buffer of words:
//!
//! - its head: the next free tile, the frame's number, the pixels that
//!   missed (`HEAD_WORDS`);
//! - the entries, the roots' and then the tiles' (`ENTRIES_AT`);
//! - the pixels that missed, a word each (`LIST_AT`).
//!
//! An entry is its key, which says it is held — the walk that wrote it, by
//! its frame and its pass (`tag`), and the zooms it was asked at — then the
//! answer's kind and block, what was handed the node after it was asked
//! (`world::At::words`), and its children's tile, taken once, by one
//! compare-and-swap. A key is one word, so a reader that trusts it trusts
//! words no walk of its frame writes again: a walk writes an entry only
//! where its key does not say what that walk asks.

use crate::answer::Kind;
use crate::world::AT_WORDS;
use crate::Zoom;

/// The head's words: the next free tile, the frame's number, and how many
/// pixels missed.
pub const NEXT_TILE: u32 = 0;
pub const FRAME: u32 = 1;
pub const MISSES: u32 = 2;
pub const HEAD_WORDS: u32 = 4;

/// The roots' slots, by a root's index's parity on each axis (`keeps`).
pub const ROOTS: u32 = 8;

/// An entry's words.
pub const W_KEY: u32 = 0;
pub const W_KIND: u32 = 1;
pub const W_BLOCK: u32 = 2;
pub const W_AT: u32 = 4;
pub const W_CHILDREN: u32 = W_AT + AT_WORDS;
pub const ENTRY_WORDS: u32 = W_CHILDREN + 1;

/// The tiles: a compromise, room enough for many frames of the about 80
/// thousand nodes a frame at E120 asks (cache/pool-probe.scratch.rs); past
/// it a node is asked again each time, as no tile is left to keep it in.
pub const TILES: u32 = 1 << 16;
pub const ENTRIES: u32 = ROOTS + 8 * TILES;
pub const ENTRIES_AT: u32 = HEAD_WORDS;
pub const LIST_AT: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;

/// No entry: a node no tile keeps.
pub const NONE: u32 = u32::MAX;

/// The frames one clearing's keys tell apart (`tag`), before the next.
pub const FRAMES: u32 = (1 << 19) - 1;

/// Entry `i`'s first word.
pub const fn entry_at(i: u32) -> u32 {
    ENTRIES_AT + i * ENTRY_WORDS
}

/// The entry of child `oct` in tile `tile`.
pub const fn child(tile: u32, oct: u32) -> u32 {
    ROOTS + 8 * tile + oct
}

/// A node's place among its parent's eight, by its index's low bits.
pub const fn octant(node: [i32; 3]) -> u32 {
    (node[0] & 1 | (node[1] & 1) << 1 | (node[2] & 1) << 2) as u32
}

/// Whether a slot keeps the root `node`: the world's roots, X's one
/// (`wrap_index` at zoom 0) and on Y and Z the root over a Q3's sign, 0 or
/// −1 (`cast::Node::index`), each in its slot by its parity (`octant`).
/// Past them, off the world, none is kept.
pub const fn keeps(node: [i32; 3]) -> bool {
    node[0] == 0 && (node[1] == 0 || node[1] == -1) && (node[2] == 0 || node[2] == -1)
}

/// The key of an entry the walk tagged `tag` wrote, asked at `[far, near]`.
pub const fn key(tag: u32, [far, near]: [Zoom; 2]) -> u32 {
    tag | far << 20 | near << 26
}

/// A walk's tag: its frame's number and its pass, the beam rays' 0 and the
/// pixels that missed 1. Never 0, which no key is.
pub const fn tag(frame: u32, pass: u32) -> u32 {
    2 * frame + pass
}

/// The tag a key holds.
pub const fn tag_of(key: u32) -> u32 {
    key & 0xF_FFFF
}

/// Whether a key says its entry holds the node asked at `asked`.
pub const fn holds(key: u32, [far, near]: [Zoom; 2]) -> bool {
    key != 0 && (key >> 20) & 63 == far && key >> 26 == near
}

/// A kind as a word, and back.
pub const fn kind_word(kind: Kind) -> u32 {
    kind as u32
}

pub const fn kind_of(w: u32) -> Kind {
    if w == Kind::Split as u32 {
        Kind::Split
    } else if w == Kind::Block as u32 {
        Kind::Block
    } else {
        Kind::Lacks
    }
}

const _: () = assert!(crate::FINEST_ZOOM < 64, "a key holds a zoom in six bits");
