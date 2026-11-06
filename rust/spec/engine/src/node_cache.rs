// pxl-engine/node_cache.rs

//! gpu-portable: the node cache, as a GPU holds it: the nodes the eye's
//! rays asked about, each the world's one query's answer at the zooms it
//! was asked at and what was handed it after, so a node is asked once, not
//! once a ray. A table of slots keyed by the node, its zoom and its index,
//! a bucket of `BUCKET` slots a key's hash (pxl-render walk_node.wgsl
//! `bucket_of`), as voxel hashing keeps its blocks (Nießner, Zollhöfer, Izadi, Stamminger 2013): a node is
//! found by its key alone, wherever the walk that asks it starts. Only nodes
//! under the world's roots are kept (pxl-render walk_node.wgsl `keeps`). What is drawn is the query's
//! answer whichever walk read it, so what is kept decides only how much is
//! asked again, never what is drawn. What is drawn is the eye's record's
//! (`record`), keyed by the same node and forgotten by this cache's rule, in
//! its pass, but kept and evicted apart from it.
//!
//! One buffer of words:
//!
//! - its head: the frame's number, what is forgotten, and the eye's
//!   clip levels' entries that moved (`HEAD_WORDS`);
//! - the entries, a slot each (`ENTRIES_AT`).
//!
//! That is all of it (`WORDS`). The pixels a frame's cast missed are
//! marked in their own rays (`ray::MISS`) and listed past the camera grid's words
//! (`ray::walked_words`) by plain writes, their count kept apart (`ray::MISS_ARGS_CNT`) and
//! copied in after (`ray::WALKED_CNT`).
//!
//! An entry is its key, which says it is held — the walk that wrote it, by
//! its frame and its pass, and the zooms it was asked at — then the
//! answer's kind and asset, what was handed the node after it was asked
//! (`world::Inheritance::words`), and the node, its zoom and its index, which tell
//! it from another node of the same slot (pxl-render forget.wgsl reads its column).
//!
//! A walk that asked the world keeps the node at the zooms asked in a slot of
//! its bucket (pxl-render ask.wgsl `keep`): a free one, the first from the
//! node's at those zooms; where none is, one an earlier frame wrote, a
//! forgotten split among them, the first from the node's own; none where every
//! slot is this frame's, or where a walk of its pass keeps the node at those
//! zooms or claims it. Which of the nodes a full bucket's walks ask are kept is
//! the order the walks run in.
//!
//! A walk writes a slot only once it has claimed it: its key exchanged for
//! `CLAIMED` and found to be the key it read. It names its claim at once, the
//! key the claim of the node at the zooms it keeps, so another walk
//! that keeps alike keeps none; writes the other words; and writes the key
//! last. A walk that found another key puts back what it took, a key over a
//! claim and a named claim over `CLAIMED` (pxl-render ask.wgsl `put_back`), and
//! tries the bucket again as it then stands. A walk that finds a claim not yet
//! named, after reading it again a few times, keeps none, as it may be its
//! own node's. A walk reads the keys it keeps by as they stand (pxl-render
//! ask.wgsl `node_cache_now`). A writing walk that read a slot takes what it read
//! only where it hashes to the entry's stamp (pxl-render ask.wgsl
//! `entry_stamp`), as another walk may be taking the slot over as it reads.

use crate::answer::Kind;
use crate::clipmap::{self, EYE_CLIP_LEVELS};
use crate::count;
use crate::overwrite::BOXES;
use crate::world::INHERITANCE_WORDS;
use crate::ZOOM_BITS;

/// The head's words: the frame's number, what this frame forgets
/// (`FORGETS_NONE`, `FORGETS_ALL`, `FORGETS_MOVED`), a bit an entry of the
/// eye's clip levels, a clip level's `clipmap::ENTRIES` bits in turn, set where
/// the brick it holds moved or was handed since the last cast, and the box those bricks lie
/// in a clip level (`MOVED_BRICKS_AT`); and the boxes of
/// leaf cells whose columns what is placed or a preview's cuts changed since
/// the last cast, how many (`BOXES_AT`) then four words each, the boxes decor's
/// generation is asked over (`overwrite::BOXES`, `overwrite::cut_box`;
/// pxl-render gpu/generator.rs); and the child groups of what is placed written
/// since the last cast, by their first slots, how many (`WRITTEN_AT`) then
/// `WRITTEN_CAP` at most, past which all is forgotten (pxl-render forget.wgsl
/// `reads_written`); and
/// whether its walks count what they ask, and what they counted (`COUNT_ON`,
/// `COUNT_AT`).
pub const FRAME: u32 = 0;
pub const FORGETS: u32 = 1;
pub const MOVED_AT: u32 = 2;
pub const MOVED_WORDS: u32 = (EYE_CLIP_LEVELS * clipmap::ENTRIES).div_ceil(32);
/// Each of the eye's clip levels' bricks that moved, the box they lie in, `[x0, y0, x1, y1]`
/// from first to last, none as `NO_BRICKS`: an entry's bit holds any brick of its place modulo
/// `clipmap::ACROSS`, the box tells them from those of other places.
pub const MOVED_BRICKS_AT: u32 = MOVED_AT + MOVED_WORDS;
pub const BOXES_AT: u32 = MOVED_BRICKS_AT + 4 * EYE_CLIP_LEVELS;
pub const WRITTEN_AT: u32 = BOXES_AT + 1 + 4 * BOXES;
pub const WRITTEN_CAP: u32 = 32;
/// Whether the walks count what they ask, where the `ray_counts` setting
/// asks, and the words they count into, each observer's walks' lanes
/// (`lane_at`).
pub const COUNT_ON: u32 = WRITTEN_AT + 1 + WRITTEN_CAP;
pub(crate) const COUNT_AT: u32 = COUNT_ON + 1;
pub(crate) const COUNT_WORDS: u32 = count::OBSERVERS * count::WALKS;
/// The missed pixels' hits kept for their faces (pxl-engine
/// `beam::HIT_WORDS`): the dispatch over them, as the walks again raise
/// it, and their count (`HITS_ARGS`, `HITS_CNT`), set each frame the cast
/// runs with the head; read, not written, by the passes it dispatches.
pub const HITS_ARGS: u32 = COUNT_AT + COUNT_WORDS;
pub const HITS_CNT: u32 = HITS_ARGS + 3;
pub const HEAD_WORDS: u32 = HITS_CNT + 1;

/// Where `observer`'s walks count lane `lane` (pxl-engine `count::BEAM`
/// to `count::MISS`) in the head.
pub const fn lane_at(observer: u32, lane: u32) -> u32 {
    COUNT_AT + observer * count::WALKS + lane
}

/// What a frame forgets: nothing; everything, where what every node is
/// asked of moved (the service area) or the keys' frames ran out; the nodes
/// over the bricks that moved, over a box of what is placed or cut that
/// changed, or handed a slot of a child group of what is placed that was
/// written (pxl-render forget.wgsl `forgets`, `over_box`, `reads_written`).
/// A clip level's moved bricks' box where none moved (`MOVED_BRICKS_AT`).
pub const NO_BRICKS: [i32; 4] = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];

/// The box of what boxes `a` and `b` hold (`MOVED_BRICKS_AT`).
pub const fn bricks_joined(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    const fn most(x: i32, y: i32) -> i32 {
        if x > y {
            x
        } else {
            y
        }
    }
    const fn least(x: i32, y: i32) -> i32 {
        if x < y {
            x
        } else {
            y
        }
    }
    [
        least(a[0], b[0]),
        least(a[1], b[1]),
        most(a[2], b[2]),
        most(a[3], b[3]),
    ]
}

pub const FORGETS_NONE: u32 = 0;
pub const FORGETS_ALL: u32 = 1;
pub const FORGETS_MOVED: u32 = 2;

/// An entry's words.
pub const W_KEY: u32 = 0;
pub const W_KIND: u32 = 1;
pub const W_ASSET: u32 = 2;
pub const W_INHERITANCE: u32 = 4;
/// The node: its zoom, then its index on X, Y and Z.
pub const W_NODE: u32 = W_INHERITANCE + INHERITANCE_WORDS;
/// What was handed it of what is placed, its `Inheritance`'s first word
/// (`world::Inheritance::words`).
pub const W_PLACED: u32 = W_INHERITANCE;
pub const ENTRY_WORDS: u32 = W_NODE + 4;

/// The slots: room for many frames of the about 80 thousand nodes a frame
/// at E120 asks; a power of two, so a key's hash masks to one.
pub const ENTRIES: u32 = 1 << 19;
pub const ENTRIES_AT: u32 = HEAD_WORDS;
/// The buffer's words, whatever is drawn: none of them a pixel's.
pub const WORDS: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;

/// No entry: a node no slot keeps.
pub const NONE: u32 = u32::MAX;

/// The bits that say a key is a named claim, and what they read in
/// one.
pub const CLAIM_MASK: u32 = CLAIM_FAR | 1 << 31;
pub const CLAIM_FAR: u32 = ((1 << ZOOM_BITS) - 1) << TAG_BITS;

/// A key one walk has claimed and not yet named: no zoom pair a
/// key holds reads as it, and no tag `FRAMES` leaves reads as a
/// forgotten split's.
pub const CLAIMED: u32 = u32::MAX;
const _: () = assert!(
    crate::FINEST_ZOOM < (1 << ZOOM_BITS) - 1 && PASSES * FRAMES + PASSES - 1 < (1 << TAG_BITS) - 1,
    "a claim holds no zoom a walk asks, and is no key"
);

/// The writing passes of a frame's cast, each a tag of its own: the
/// beam rays, the samples and the pixels that missed; then the light's beam
/// rays, samples and slots that missed, which ask the same nodes at the
/// zooms the eye asks them.
pub const PASSES: u32 = 6;
pub const BEAM_PASS: u32 = 0;
pub const SAMPLES_PASS: u32 = 1;
pub const MISSED_PASS: u32 = 2;
pub const LIGHT_BEAM_PASS: u32 = 3;
pub const LIGHT_SAMPLES_PASS: u32 = 4;
pub const LIGHT_MISSED_PASS: u32 = 5;

/// A key's tag's bits, under its two zooms'.
pub const TAG_BITS: u32 = 20;
const _: () = assert!(
    TAG_BITS + 2 * ZOOM_BITS == 32,
    "a key is the tag and two zooms"
);

/// The frames one clearing's keys tell apart, before the next: as
/// many as the tag's `TAG_BITS` hold `PASSES` tags of.
pub const FRAMES: u32 = (1 << TAG_BITS) / PASSES - 1;

/// The slots a bucket holds: a node is kept in any of its bucket's, so two
/// nodes asked every frame whose hashes meet are both kept.
pub const BUCKET: u32 = 4;

/// The bits of an entry's kind word (`W_KIND`) its kind takes, under the
/// stamp that hashes the entry's words.
pub const KIND_MASK: u32 = 3;
const _: () = assert!(
    Kind::Split as u32 <= KIND_MASK
        && Kind::Cell as u32 <= KIND_MASK
        && Kind::Lacks as u32 <= KIND_MASK,
    "a kind fits under the stamp"
);

const _: () = assert!(
    crate::FINEST_ZOOM < 1 << ZOOM_BITS,
    "a key holds a zoom in its bits"
);
