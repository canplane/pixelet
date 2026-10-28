// pxl-spec-core/observed.rs

//! gpu-portable: the observed pyramid, as a GPU holds it: the nodes the eye's
//! rays asked about, each the world's one query's answer at the zooms it
//! was asked at and what was handed it after, so a node is asked once, not
//! once a ray. A table of slots keyed by the node, its zoom and its index,
//! a bucket of `BUCKET` slots a key's hash (`entry_of`), as voxel hashing
//! keeps its blocks (Nießner, Zollhöfer, Izadi, Stamminger 2013): a node is
//! found by its key alone, wherever the walk that asks it starts, in a free
//! slot of its bucket, or where none is, taking one over (`victim`). Only nodes under the world's roots are kept (`keeps`). What
//! is drawn is the query's answer whichever walk read it, so what is kept
//! decides only how much is asked again, never what is drawn.
//!
//! One buffer of words:
//!
//! - its head: the frame's number, what is forgotten, and the eye's
//!   windows' slots that moved (`HEAD_WORDS`);
//! - the entries, a slot each (`ENTRIES_AT`);
//! - the pixels the frame's reading walk marked missed, listed for the walk
//!   that writes the pyramid (`MARKS_AT`).
//!
//! That is all of it (`WORDS`). The pixels a frame's cast missed are
//! marked in their own rays (`ray::MISS`) and listed here by plain writes,
//! their count kept apart (`ray::MISS_ARGS_COUNT`) and copied in after
//! (`MARKS_COUNT`), so a walk that only reads the pyramid types no word of
//! it atomic.
//!
//! An entry is its key, which says it is held — the walk that wrote it, by
//! its frame and its pass (`tag`), and the zooms it was asked at — then the
//! answer's kind and asset, what was handed the node after it was asked
//! (`world::At::words`), and the node, its zoom and its index, which tell
//! it from another node of the same slot (`forgets` reads its column). A
//! walk writes a slot only once it has claimed it, its key swapped to
//! `CLAIMED` in one compare-and-swap, and writes the key last; a writing
//! walk that read a slot reads its key again after, and takes what it read
//! only where the key stood, as another walk may have taken the slot over
//! in between.

use crate::answer::Kind;
use crate::overwrite::{cut_box, BOXES};
use crate::tally;
use crate::window::{eye_cell, slot, ACROSS, EYE_WINDOWS, SLOTS, SQUARE_SHIFT};
use crate::world::AT_WORDS;
use crate::{wrap_index, Zoom, LEAF_CELL, ZOOM_BITS};

/// The head's words: the frame's number, what this frame forgets
/// (`FORGETS_NONE`, `FORGETS_ALL`, `FORGETS_MOVED`), a bit a slot of the
/// eye's windows, a window's `SLOTS` bits in turn, set where the square it
/// holds moved or was handed since the last cast; and the boxes of leaf
/// cells whose columns what is placed or a preview's cuts changed since the
/// last cast, how many (`BOXES_AT`) then four words each, the boxes decor's
/// growth is asked over (`overwrite::BOXES`, `overwrite::cut_box`;
/// pxl-render gpu/grown.rs); and the families of what is placed written
/// since the last cast, by their first slots, how many (`WRITTEN_AT`) then
/// `WRITTEN` at most, past which all is forgotten (`reads_written`); and
/// whether its walks count what they ask, and what they counted
/// (`TALLY_ON`, `TALLY_AT`).
pub const FRAME: u32 = 0;
pub const FORGETS: u32 = 1;
pub const MOVED_AT: u32 = 2;
pub const MOVED_WORDS: u32 = (EYE_WINDOWS * SLOTS).div_ceil(32);
pub const BOXES_AT: u32 = MOVED_AT + MOVED_WORDS;
pub const WRITTEN_AT: u32 = BOXES_AT + 1 + 4 * BOXES;
pub const WRITTEN: u32 = 32;
/// Whether the walks count what they ask, where the `ray_counts` setting
/// asks, and the words they count into, each observer's walks' lanes
/// (`tally_at`).
pub const TALLY_ON: u32 = WRITTEN_AT + 1 + WRITTEN;
pub const TALLY_AT: u32 = TALLY_ON + 1;
pub const TALLY_WORDS: u32 = tally::OBSERVERS * tally::WALKS;
pub const HEAD_WORDS: u32 = TALLY_AT + TALLY_WORDS;

/// Where `observer`'s walks count lane `lane` (pxl-spec-core `tally::BEAM`
/// to `tally::MISS`) in the head.
pub const fn tally_at(observer: u32, lane: u32) -> u32 {
    TALLY_AT + observer * tally::WALKS + lane
}

/// What a frame forgets: nothing; everything, where what every node is
/// asked of moved (the service area) or the keys' frames ran out; the nodes
/// over the squares that moved (`forgets`), over a box of what is placed or
/// cut that changed (`over_box`), or handed a slot of a family of what is
/// placed that was written (`reads_written`).
pub const FORGETS_NONE: u32 = 0;
pub const FORGETS_ALL: u32 = 1;
pub const FORGETS_MOVED: u32 = 2;

/// An entry's words.
pub const W_KEY: u32 = 0;
pub const W_KIND: u32 = 1;
pub const W_ASSET: u32 = 2;
pub const W_AT: u32 = 4;
/// The node: its zoom, then its index on X, Y and Z.
pub const W_NODE: u32 = W_AT + AT_WORDS;
/// What was handed it of what is placed, its `At`'s second word
/// (`world::At::words`).
pub const W_PLACED: u32 = W_AT + 1;
pub const ENTRY_WORDS: u32 = W_NODE + 4;

/// The slots: no more than the tiles' entries were (8 roots and 2^16 tiles
/// of eight), room for many frames of the about 80 thousand nodes a frame
/// at E120 asks (a probe's count, not kept); a power of two, so a key's
/// hash masks to one.
pub const ENTRIES: u32 = 1 << 19;
pub const ENTRIES_AT: u32 = HEAD_WORDS;
/// The marked pixels listed (`MARKS_AT`), a word each, its row over its
/// column, as many as `MARKS`; and their count, copied in from the walk's
/// dispatch (`ray::MISS_ARGS_COUNT`) once the reading walk is done
/// (`MARKS_COUNT`). A pixel past `MARKS` is walked by the walk over every
/// pixel (`ray::MISS_ARGS_WHOLE`). Room for the bursts a load or a long
/// zoom marks, about a hundred thousand at 9 Mpx.
pub const MARKS: u32 = 1 << 20;
pub const MARKS_COUNT: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;
pub const MARKS_AT: u32 = MARKS_COUNT + 1;
/// The buffer's words, whatever is drawn: the list's room fixed, so none
/// of them a pixel's.
pub const WORDS: u32 = MARKS_AT + MARKS;

/// No entry: a node no slot keeps.
pub const NONE: u32 = u32::MAX;

/// A key one walk has claimed and is writing the slot under: no zoom
/// pair a key holds reads as it (`holds`).
pub const CLAIMED: u32 = u32::MAX;
const _: () = assert!(
    crate::FINEST_ZOOM < 63,
    "a claimed key holds no zoom a walk asks"
);

/// The writing passes of a frame's cast, each a tag of its own (`tag`): the
/// beam rays, the samples and the pixels that missed; then the sun's beam
/// rays, samples and slots that missed, which ask the same nodes at the
/// zooms the eye asks them; and the eye's line, asked ahead of its beam
/// rays, which every one of them enters first (cast.rs `origin_line`).
pub const PASSES: u32 = 7;
pub const BEAM_PASS: u32 = 0;
pub const SAMPLES_PASS: u32 = 1;
pub const MISSED_PASS: u32 = 2;
pub const SUN_BEAM_PASS: u32 = 3;
pub const SUN_SAMPLES_PASS: u32 = 4;
pub const SUN_MISSED_PASS: u32 = 5;
pub const ORIGIN_PASS: u32 = 6;

/// A key's tag's bits, under its two zooms' (`key`).
const TAG_BITS: u32 = 20;
const _: () = assert!(
    TAG_BITS + 2 * ZOOM_BITS == 32,
    "a key is the tag and two zooms"
);

/// The frames one clearing's keys tell apart (`tag`), before the next: as
/// many as the tag's `TAG_BITS` hold `PASSES` tags of.
pub const FRAMES: u32 = (1 << TAG_BITS) / PASSES - 1;

/// Entry `i`'s first word.
pub const fn entry_at(i: u32) -> u32 {
    ENTRIES_AT + i * ENTRY_WORDS
}

/// Whether a slot may keep the node `node` of `zoom`: one under the
/// world's roots, X's one (`wrap_index` at zoom 0) and on Y and Z the root
/// over a Q3's sign, 0 or −1 (`cast::Node::index`). Past them, off the
/// world, none is kept.
pub fn keeps(zoom: Zoom, node: [i32; 3]) -> bool {
    // A node past `UNIT_ZOOM` is finer than any walk asks; an i32 shifted by
    // 32 would read as unshifted.
    if zoom >= 32 {
        return false;
    }
    let (x, y, z) = (node[0] >> zoom, node[1] >> zoom, node[2] >> zoom);
    x == 0 && (y == 0 || y == -1) && (z == 0 || z == -1)
}

/// The slots a bucket holds: a node is kept in any of its bucket's, so two
/// nodes asked every frame whose hashes meet are both kept.
pub const BUCKET: u32 = 4;

/// The first entry of the bucket of the node `node` of `zoom`: its key's
/// hash, masked to the table, the same for every walk that asks it.
pub fn entry_of(zoom: Zoom, node: [i32; 3]) -> u32 {
    hash(zoom, node) & (ENTRIES - 1) & !(BUCKET - 1)
}

/// Which slot of its bucket a node takes over where none is free: one by
/// its hash, so two nodes of a bucket take over different slots.
pub fn victim(zoom: Zoom, node: [i32; 3]) -> u32 {
    hash(zoom, node) >> 28 & (BUCKET - 1)
}

fn hash(zoom: Zoom, node: [i32; 3]) -> u32 {
    let mut h = zoom.wrapping_mul(0x85EB_CA77);
    h = mix(h, node[0] as u32);
    h = mix(h, node[1] as u32);
    h = mix(h, node[2] as u32);
    h ^ h >> 16
}

/// One word of a key folded into a hash.
fn mix(h: u32, v: u32) -> u32 {
    (h ^ v).wrapping_mul(0x9E37_79B1).rotate_left(15)
}

/// The key of an entry the walk tagged `tag` wrote, asked at `[far, near]`.
pub const fn key(tag: u32, [far, near]: [Zoom; 2]) -> u32 {
    tag | far << TAG_BITS | near << (TAG_BITS + ZOOM_BITS)
}

/// A walk's tag: its frame's number and its pass (`PASSES`). Never 0, which
/// no key is.
pub const fn tag(frame: u32, pass: u32) -> u32 {
    PASSES * frame + pass
}

/// The tag a key holds.
pub const fn tag_of(key: u32) -> u32 {
    key & ((1 << TAG_BITS) - 1)
}

/// Whether a key says its entry holds the node asked at `asked`.
pub const fn holds(key: u32, [far, near]: [Zoom; 2]) -> bool {
    key != 0
        && (key >> TAG_BITS) & ((1 << ZOOM_BITS) - 1) == far
        && key >> (TAG_BITS + ZOOM_BITS) == near
}

/// A kind as a word, and back.
pub const fn kind_word(kind: Kind) -> u32 {
    kind as u32
}

pub const fn kind_of(w: u32) -> Kind {
    if w == Kind::Split as u32 {
        Kind::Split
    } else if w == Kind::Cell as u32 {
        Kind::Cell
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
/// Whether the node of `zoom` at column `[x, y]` stands over the box `b` of
/// leaf cells `[x0, y0, x1, y1)`, X round the world: its own columns as a
/// box of leaf cells (`overwrite::cut_box`) meet it. A node's answer, and what
/// was handed it, read what is placed in its own columns and its
/// ancestors' alone, so a node no changed box meets answers as it did.
pub fn over_box(zoom: Zoom, [x, y]: [i32; 2], b: [i32; 4]) -> bool {
    let n = cut_box(zoom, [x, y, 0]);
    let world = 1i32 << LEAF_CELL;
    let (nw, bw) = (n[2] - n[0], b[2] - b[0]);
    // The box's first column from the node's, the short way round.
    let from = wrap_index(LEAF_CELL, b[0].wrapping_sub(n[0]));
    let across = nw >= world
        || bw >= world
        || (from < nw && from + bw > 0)
        || from + bw > world
        || from + world < nw;
    across && b[1] < n[3] && b[3] > n[1]
}

/// Whether what was handed a node of what is placed, its `At`'s word
/// `placed` (`overwrite::Held`), answers from a slot of the family written
/// whose first slot is `first`. A write folds up its own path in place, and
/// that path's nodes stand over its box (`over_box`); but where it splits a
/// leaf that covered wider than the box, or takes a family a join let go of,
/// a node beside the box holds a slot the write changed, and reads another
/// node's what is placed after it.
pub fn reads_written(placed: u32, first: u32) -> bool {
    let held = crate::overwrite::Held::of_word(placed);
    held.holds() && held.slot().wrapping_sub(first) < crate::overwrite::FAMILY
}

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
        let (q0, q1) = (
            [lo[0] >> SQUARE_SHIFT, lo[1] >> SQUARE_SHIFT],
            [hi[0] >> SQUARE_SHIFT, hi[1] >> SQUARE_SHIFT],
        );
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

const _: () = assert!(
    crate::FINEST_ZOOM < 1 << ZOOM_BITS,
    "a key holds a zoom in its bits"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// A node stands over a box where their columns meet, X round the
    /// world, and not beside it.
    #[test]
    fn a_node_stands_over_the_boxes_its_columns_meet() {
        // A leaf cell's own node, and the node four cells a side holding it.
        let (leaf, wide) = (LEAF_CELL, LEAF_CELL - 2);
        let b = [10, 20, 11, 21];
        assert!(over_box(leaf, [10, 20], b));
        assert!(!over_box(leaf, [11, 20], b), "beside it across");
        assert!(!over_box(leaf, [10, 21], b), "beside it down");
        assert!(over_box(wide, [2, 5], b), "the node holding it");
        assert!(!over_box(wide, [3, 5], b));
        // A box over the world's last column and its first, round the seam.
        let world = 1i32 << LEAF_CELL;
        let seam = [world - 1, 0, world + 1, 1];
        assert!(over_box(leaf, [0, 0], seam));
        assert!(over_box(leaf, [-1, 0], seam));
        assert!(!over_box(leaf, [1, 0], seam));
        // The world's root holds every box.
        assert!(over_box(0, [0, 0], b));
    }

    /// A node reads a written family where what was handed it answers from
    /// one of that family's slots, and not where it holds nothing.
    #[test]
    fn a_node_reads_the_families_its_slot_lies_in() {
        use crate::overwrite::{Held, FAMILY};
        let held = |slot: u32| Held::branch(slot, 9).word();
        assert!(reads_written(held(40), 40));
        assert!(reads_written(held(40 + FAMILY - 1), 40));
        assert!(!reads_written(held(40 + FAMILY), 40));
        assert!(!reads_written(held(39), 40));
        assert!(!reads_written(Held::NOTHING.word(), 0));
    }
}
