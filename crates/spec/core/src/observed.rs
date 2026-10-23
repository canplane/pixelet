// pxl-spec-core/observed.rs

//! gpu-portable: the observed pyramid, as a GPU holds it: the nodes the eye's
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
//! - its head: how many tiles are taken, the frame's number, what is
//!   forgotten, and the eye's windows' slots that moved (`HEAD_WORDS`);
//! - the entries, the roots' and then the tiles' (`ENTRIES_AT`);
//! - the tiles' stack, the free ones past those taken (`STACK_AT`).
//!
//! That is all of it (`WORDS`): the pixels a frame's cast missed are the
//! cast's own for that frame, in the beam rays' buffer (`beam::list_at`),
//! so a walk that only reads the pyramid holds its binding read-only, and
//! a GPU's compiler types no word of it atomic for that walk.
//!
//! An entry is its key, which says it is held — the walk that wrote it, by
//! its frame and its pass (`tag`), and the zooms it was asked at — then the
//! answer's kind and block, what was handed the node after it was asked
//! (`world::At::words`), its children's tile, taken once, by one
//! compare-and-swap, and the node, its zoom and its column (`forgets`). A key is one word, so a reader that trusts it trusts
//! words no walk of its frame writes again: a walk writes an entry only
//! where its key does not say what that walk asks.
//!
//! A tile goes back on the stack once it keeps nothing: each of its eight
//! forgotten, its children's tile gone back before it. Its parent gives it
//! back, the one word that names it, so a tile is never given back twice;
//! the walks take tiles and the forgetting gives them back, in dispatches
//! of their own, so the stack is never taken from and given to at once.

use crate::answer::Kind;
use crate::window::{eye_cell, slot, ACROSS, EYE_WINDOWS, SLOTS, SQUARE_SHIFT};
use crate::world::AT_WORDS;
use crate::Zoom;

/// The head's words: how many tiles are taken, the frame's number, what
/// this frame forgets (`FORGETS_NONE`, `FORGETS_ALL`, `FORGETS_MOVED`), and
/// a bit a slot of the eye's windows, a window's `SLOTS` bits in turn, set
/// where the square it holds moved or was handed since the last cast.
pub const TAKEN: u32 = 0;
pub const FRAME: u32 = 1;
pub const FORGETS: u32 = 2;
pub const MOVED_AT: u32 = 3;
pub const MOVED_WORDS: u32 = (EYE_WINDOWS * SLOTS).div_ceil(32);
pub const HEAD_WORDS: u32 = MOVED_AT + MOVED_WORDS;

/// What a frame forgets: nothing; everything, where what every node is
/// asked of moved (the service area, what is placed, a preview's cuts) or
/// the keys' frames ran out; the nodes over the squares that moved
/// (`forgets`).
pub const FORGETS_NONE: u32 = 0;
pub const FORGETS_ALL: u32 = 1;
pub const FORGETS_MOVED: u32 = 2;

/// The roots' slots, by a root's index's parity on each axis (`keeps`).
pub const ROOTS: u32 = 8;

/// An entry's words.
pub const W_KEY: u32 = 0;
pub const W_KIND: u32 = 1;
pub const W_BLOCK: u32 = 2;
pub const W_AT: u32 = 4;
pub const W_CHILDREN: u32 = W_AT + AT_WORDS;
pub const W_NODE: u32 = W_CHILDREN + 1;
pub const ENTRY_WORDS: u32 = W_NODE + 3;

/// The tiles: a compromise, room enough for many frames of the about 80
/// thousand nodes a frame at E120 asks (a probe's count, not kept); past
/// it a node is asked again each time, as no tile is left to keep it in.
pub const TILES: u32 = 1 << 16;
pub const ENTRIES: u32 = ROOTS + 8 * TILES;
pub const ENTRIES_AT: u32 = HEAD_WORDS;
/// The tiles' stack, a tile a word: those taken below `TAKEN`, the free
/// ones from it up, the next taken first; every tile in turn where the
/// pyramid is forgotten whole.
pub const STACK_AT: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;
/// The buffer's words, whatever is drawn: none of them a pixel's.
pub const WORDS: u32 = STACK_AT + TILES;

/// No entry: a node no tile keeps.
pub const NONE: u32 = u32::MAX;

/// A children's word one walk has claimed and not yet given its tile.
pub const CLAIMED: u32 = u32::MAX;

/// The writing passes of a frame's cast, each a tag of its own (`tag`): the
/// beam rays, the samples and the pixels that missed.
pub const PASSES: u32 = 3;

/// The frames one clearing's keys tell apart (`tag`), before the next: as
/// many as the tag's 20 bits hold `PASSES` tags of.
pub const FRAMES: u32 = (1 << 20) / PASSES - 1;

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

/// A walk's tag: its frame's number and its pass, the beam rays' 0, the
/// samples' 1 and the pixels that missed 2 (`PASSES`). Never 0, which no
/// key is.
pub const fn tag(frame: u32, pass: u32) -> u32 {
    PASSES * frame + pass
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

/// Whether the node of `zoom` at column `[x, y]` reads a square that moved,
/// `moved` the moved slots' words (`MOVED_AT`), as a walk holds them, never
/// a closure over the buffer, which rust-gpu reads wrongly: a node's answer reads
/// the squares under it alone — its cells' and their windows' over them
/// (`window::step`), a wider node's the block of squares its own size
/// (`window::bounded`) — and what was handed it found its cells there, so a
/// node no moved square lies under answers as it did. A node a window's
/// whole side wide or wider reads every slot of the window.
pub fn forgets(moved: &[u32; MOVED_WORDS as usize], zoom: Zoom, [x, y]: [i32; 2]) -> bool {
    let mut w = 0;
    while w < EYE_WINDOWS {
        let c = eye_cell(w);
        // The node's cells at the window's zoom, first and last, then its
        // squares.
        let (lo, hi) = if zoom <= c {
            let s = c - zoom;
            ([x << s, y << s], [((x + 1) << s) - 1, ((y + 1) << s) - 1])
        } else {
            let s = zoom - c;
            ([x >> s, y >> s], [x >> s, y >> s])
        };
        let (q0, q1) = ([lo[0] >> SQUARE_SHIFT, lo[1] >> SQUARE_SHIFT], [hi[0] >> SQUARE_SHIFT, hi[1] >> SQUARE_SHIFT]);
        // As wide as the window or wider, every slot; else each square.
        let wide = q1[0] - q0[0] + 1 >= ACROSS as i32 || q1[1] - q0[1] + 1 >= ACROSS as i32;
        if wide {
            let mut k = 0;
            while k < SLOTS {
                if moved_in(moved, w, k) {
                    return true;
                }
                k += 1;
            }
        } else {
            let mut qy = q0[1];
            while qy <= q1[1] {
                let mut qx = q0[0];
                while qx <= q1[0] {
                    if moved_in(moved, w, slot([qx, qy])) {
                        return true;
                    }
                    qx += 1;
                }
                qy += 1;
            }
        }
        w += 1;
    }
    false
}

/// Whether slot `slot` of the eye's window `w` moved, among the moved
/// slots' words.
pub const fn moved_in(moved: &[u32; MOVED_WORDS as usize], w: u32, slot: u32) -> bool {
    let k = w * SLOTS + slot;
    moved[(k / 32) as usize] & 1 << (k % 32) != 0
}

const _: () = assert!(crate::FINEST_ZOOM < 64, "a key holds a zoom in six bits");
