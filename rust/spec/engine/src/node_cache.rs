// pxl-engine/node_cache.rs

//! gpu-portable: the node cache, as a GPU holds it: the nodes the eye's
//! rays asked about, each the world's one query's answer at the zooms it
//! was asked at and what was handed it after, so a node is asked once, not
//! once a ray. A table of slots keyed by the node, its zoom and its index,
//! a bucket of `BUCKET` slots a key's hash (pxl-render walk_node.wgsl
//! `bucket_of`), as voxel hashing keeps its blocks (Nießner, Zollhöfer, Izadi, Stamminger 2013): a node is
//! found by its key alone, wherever the walk that asks it starts. Only nodes
//! under the world's roots are kept (`keeps`). What is drawn is its drawable entries
//! (`record`): the cells a ray stopped on, each the query's answer for its node, which takes
//! no zooms asked (world.test.rs `cell_answer_takes_no_zooms`), and the faces rays entered
//! it by. What is kept decides how much is asked again and, through its drawable entries,
//! what is drawn; no entry drawn in the last two frames is evicted (pxl-render ask.wgsl
//! `claim_slot`, `drawn`).
//!
//! One buffer of words:
//!
//! - its head: the frame's number, what is forgotten, and the eye's
//!   clip levels' entries that moved (`HEADER_WORDS`);
//! - the frame the eye's draw last ran (`DRAWN`);
//! - the keys, a word a slot, a bucket's side by side (`KEYS_AT`);
//! - the states, a word a slot (`STATES_AT`);
//! - the entries, a slot each (`ENTRIES_AT`);
//! - the records, what else draws a slot (`RECORDS_AT`).
//!
//! That is all of it (`WORDS`). The pixels a frame's cast missed are
//! marked past the camera grid's words, a bit a pixel (`ray::holes_words`).
//!
//! A key says its entry is held: the walk that wrote it, by its frame and its pass, and the
//! zooms it keeps the node at (`key_zooms`). An entry is the answer's kind and asset, what was handed the node after it was asked
//! (`world::Inheritance::words`), and the node, its zoom and its index, which tell
//! it from another node of the same slot (pxl-render forget.wgsl reads its column); then
//! what draws it lies apart, none until a ray stops on it (`STATES_AT`, `RECORDS_AT`), written by
//! the stops, the draws and the faces' pass, and set to none where the slot is written: a walk
//! reads a bucket's keys off one line and an entry's words off another, and a pass over every
//! slot reads the words it asks.
//!
//! **One fact, one entry:** a node the query answers by its cell (pxl-render world_split.wgsl
//! `world_node`: asked at its zoom or coarser, or whole) has one answer and hands on the same
//! words whatever zooms it and every node over it were asked at (world.test.rs
//! `cell_answer_takes_no_zooms`). Such a cell is kept by its node alone, its key's far zoom
//! `CELL_FAR`, its near zoom 1 where it was found whole and 0 else (`key_zooms`); a split, or
//! a void a split's rule answered, which another pair may split, is kept at the pair asked. A
//! walk asking a node at any pair takes the slot that keeps it at that pair, or keeps it as a
//! cell its query would answer there too: asked at the cell's zoom or coarser, or the cell
//! whole (`holds_for`; pxl-render ask.wgsl `ask_node`, `kept_at`, cached.wgsl
//! `node_answer`). The two never both hold a node for one pair: where the query answers a
//! node its cell at a pair, no split's rule answers it there. A walk that finds a cell whole,
//! kept already but not whole, sets it whole (ask.wgsl `holds_cell`); its stamp takes no
//! whole (`entry_stamp`). Forget, what draws an entry and eviction read the node and the
//! frame, not the zooms, and are the same for either key.
//!
//! A walk that asked the world keeps the node at the zooms its answer is kept at in a slot of
//! its bucket (pxl-render ask.wgsl `keep`): a free one, the first from the
//! node's at those zooms; where none is, one an earlier frame wrote and no draw
//! drew in the last two frames (`record::drawn`), a forgotten split among them,
//! the first from the node's own; none where every slot is this frame's or drawn,
//! or where a walk of its pass keeps the node at those zooms or claims it. Which of the nodes a full bucket's walks ask are kept is
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
use crate::ground::VOID;
use crate::object::RECTS;
use crate::world::INHERITANCE_WORDS;
use crate::{Zoom, ZOOM_BITS};

/// The head's words: the frame's number, what this frame forgets
/// (`FORGETS_NONE`, `FORGETS_ALL`, `FORGETS_CHANGED`), a bit an entry of the
/// eye's clip levels, a clip level's `clipmap::ENTRIES` bits in turn, set where
/// the brick it holds moved or was handed since the last cast, and the box those bricks lie
/// in a clip level (`CHANGED_BRICKS_AT`); and the boxes of
/// leaf cells whose columns what is placed or a preview's cuts changed since
/// the last cast, how many (`RECTS_AT`) then four words each, the boxes decor's
/// generation is asked over (`object::RECTS`, `object::cut_rect`;
/// pxl-render gpu/generator.rs); and the child groups of what is placed written
/// since the last cast, by their first slots, how many (`WRITTEN_AT`) then
/// `WRITTEN_CAP` at most, past which all is forgotten (pxl-render forget.wgsl
/// `reads_written`); and
/// whether its walks count what they ask, and what they counted (`COUNT_ON`,
/// `COUNT_AT`).
pub const FRAME: u32 = 0;
pub const FORGETS: u32 = 1;
pub const CHANGED_AT: u32 = 2;
pub const CHANGED_WORDS: u32 = (EYE_CLIP_LEVELS * clipmap::ENTRIES).div_ceil(32);
/// Each of the eye's clip levels' bricks that moved held present (`Entry::held_changed_from`), the
/// box they lie in, `[x0, y0, x1, y1]` from first to last, none as `NO_BRICKS`: an entry's bit
/// holds any brick of its place modulo `clipmap::ACROSS`, the box tells them from those of other
/// places, for the nodes that read bricks held present alone (pxl-render forget.wgsl `forgets`).
pub const CHANGED_BRICKS_AT: u32 = CHANGED_AT + CHANGED_WORDS;
pub const RECTS_AT: u32 = CHANGED_BRICKS_AT + 4 * EYE_CLIP_LEVELS;
pub const WRITTEN_AT: u32 = RECTS_AT + 1 + 4 * RECTS;
pub const WRITTEN_CAP: u32 = 32;
/// Which observers' walks and pixels count what they ask, a bit an observer (`count::EYE`,
/// `count::LIGHT`), set where the `ray_counts` setting asks and the observer casts, and the
/// words they count into, each observer's walks' lanes (`lane_at`).
pub const COUNT_ON: u32 = WRITTEN_AT + 1 + WRITTEN_CAP;
pub(crate) const COUNT_AT: u32 = COUNT_ON + 1;
pub(crate) const COUNT_WORDS: u32 = count::OBSERVERS * count::WALKS;
/// The missed pixels' hits kept for their faces (pxl-engine
/// `beam::HIT_WORDS`): the dispatch over them, as the walks again raise
/// it, and their count (`HITS_ARGS`, `HITS_CNT`), set each frame the cast
/// runs with the head; read, not written, by the passes it dispatches.
pub const HITS_ARGS: u32 = COUNT_AT + COUNT_WORDS;
pub const HITS_CNT: u32 = HITS_ARGS + 3;
/// The next ray the holes' queue's last walk takes (pxl-render queue_last.wgsl), each
/// observer's in turn, set back to 0 by the asks that run before it (queue_ask.wgsl).
pub const LAST_NEXT: u32 = HITS_CNT + 1;
/// The nodes the frame asked of the world so far, every run's together (`ray::ASKS_A_FRAME`):
/// set to none by the host each frame a run runs.
pub const ASKED: u32 = LAST_NEXT + 1;
pub const HEADER_WORDS: u32 = ASKED + 1;
/// The frame the eye's draw last ran, written by it (pxl-render record_draw.wgsl
/// `record_draw_cs`): the boxes the raster draws until it runs again name the entries it
/// marked seen then (`record::drawn`). Past the head the host writes each frame.
pub const DRAWN: u32 = HEADER_WORDS;
/// Where `observer`'s walks count lane `lane` (pxl-engine `count::BEAM`
/// to `count::MISS`) in the head.
pub const fn lane_at(observer: u32, lane: u32) -> u32 {
    COUNT_AT + observer * count::WALKS + lane
}

/// What a frame forgets: nothing; everything, where what every node is
/// asked of moved (the service area) or the keys' frames ran out; the nodes
/// over the bricks that moved, over a box of what is placed or cut that
/// changed, or handed a slot of a child group of what is placed that was
/// written (pxl-render forget.wgsl `forgets`, `over_rect`, `reads_written`).
/// A clip level's moved bricks' box where none moved (`CHANGED_BRICKS_AT`).
pub const NO_BRICKS: [i32; 4] = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];

/// The box of what boxes `a` and `b` hold (`CHANGED_BRICKS_AT`).
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
pub const FORGETS_CHANGED: u32 = 2;

/// An entry's words.
pub const W_KIND: u32 = 0;
pub const W_ASSET: u32 = 1;
pub const W_INHERITANCE: u32 = 3;
/// The node: its zoom, then its index on X, Y and Z.
pub const W_NODE: u32 = W_INHERITANCE + INHERITANCE_WORDS;
/// What was handed it of what is placed, its `Inheritance`'s first word
/// (`world::Inheritance::words`).
pub const W_PLACED: u32 = W_INHERITANCE;
pub const ENTRY_WORDS: u32 = W_NODE + 4;

/// A slot's record: the last frame a draw drew it (`record::drawn`), and what lies about each
/// face, a word a side, as `ray::face` numbers them (`face::word`). Its state lies apart
/// (`STATES_AT`): the faces rays entered and asked about and whether some ray stopped on it whole
/// (`record::FACES`, `record::ASKED`, `record::WHOLE`).
pub const R_DRAWN: u32 = 0;
pub const R_FACES: u32 = 1;
pub const R_WORDS: u32 = R_FACES + 6;

/// The slots, a power of two, so a key's hash masks to one: what the table must hold, derived
/// in the record that draws its entries (`record::entries_for`, which asserts it).
pub const ENTRIES: u32 = 1 << 20;
pub const KEYS_AT: u32 = DRAWN + 1;
pub const STATES_AT: u32 = KEYS_AT + ENTRIES;
pub const ENTRIES_AT: u32 = STATES_AT + ENTRIES;
pub const RECORDS_AT: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;
/// The buffer's words, whatever is drawn: none of them a pixel's.
pub const WORDS: u32 = RECORDS_AT + ENTRIES * R_WORDS;
const _: () = assert!(
    (WORDS as u64) * 4 <= 134_217_728,
    "the node cache binds whole as storage on every device"
);

/// The first word of slot `slot`'s entry.
pub const fn entry_at(slot: u32) -> u32 {
    ENTRIES_AT + slot * ENTRY_WORDS
}

/// Slot `slot`'s key.
pub const fn key_at(slot: u32) -> u32 {
    KEYS_AT + slot
}

/// Slot `slot`'s state.
pub const fn state_at(slot: u32) -> u32 {
    STATES_AT + slot
}

/// The last frame a draw drew slot `slot`.
pub const fn drawn_at(slot: u32) -> u32 {
    RECORDS_AT + slot * R_WORDS + R_DRAWN
}

/// What lies about slot `slot`'s face `face`.
pub const fn face_at(slot: u32, face: u32) -> u32 {
    RECORDS_AT + slot * R_WORDS + R_FACES + face
}

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

/// The far zoom of the key of a cell kept by its node alone, the zooms its key holds this and
/// its whole bit over `ZOOM_BITS` (`key_zooms`): no zoom a walk asks, and no claim's.
pub const CELL_FAR: u32 = (1 << ZOOM_BITS) - 2;
const _: () = assert!(
    crate::FINEST_ZOOM < CELL_FAR && CELL_FAR << TAG_BITS != CLAIM_FAR,
    "a cell's key holds no zoom pair a walk asks, and is no claim"
);

/// The zooms, far then near over `ZOOM_BITS`, a key keeps the node of `zoom`, asked at
/// `zooms`, at where the query answered it `kind` and `asset` (pxl-render ask.wgsl
/// `key_zooms`): a cell its query answers by its cell, by its node alone (`CELL_FAR`), whole
/// where it was answered coarser than asked, which no split's rule answers but void; else the
/// zooms asked.
pub const fn key_zooms(zoom: Zoom, zooms: u32, kind: Kind, asset: [u32; 2]) -> u32 {
    if matches!(kind, Kind::Split) {
        return zooms;
    }
    if zoom >= zooms >> ZOOM_BITS {
        return CELL_FAR;
    }
    if matches!(kind, Kind::Lacks) || asset[0] != VOID || asset[1] != 0 {
        return CELL_FAR | 1 << ZOOM_BITS;
    }
    zooms
}

/// Whether a held key `key` keeps its node, of `zoom`, for a walk asking it at `zooms` (pxl-render
/// walk_node.wgsl `holds_for`): kept at those zooms, or a cell kept by its node alone that the
/// query answers its cell at these zooms too, asked at its zoom or coarser, or whole.
pub const fn holds_for(key: u32, zoom: Zoom, zooms: u32) -> bool {
    let kz = key >> TAG_BITS;
    kz == zooms
        || kz & ((1 << ZOOM_BITS) - 1) == CELL_FAR
            && (zooms >> ZOOM_BITS <= zoom || kz >> ZOOM_BITS != 0)
}
const _: () = assert!(
    crate::FINEST_ZOOM < (1 << ZOOM_BITS) - 1 && PASSES * FRAMES + PASSES - 1 < (1 << TAG_BITS) - 1,
    "a claim holds no zoom a walk asks, and is no key"
);

/// The writing passes of a frame's cast, each a tag of its own: the
/// beam rays and the samples; the holes' queue, the eye's pixels and the
/// light's slots alike (pxl-render queue_ask.wgsl, queue_last.wgsl); then the
/// light's beam rays and samples, which ask the same nodes at the zooms the
/// eye asks them.
pub const PASSES: u32 = 5;
pub const BEAM_PASS: u32 = 0;
pub const SAMPLES_PASS: u32 = 1;
pub const MISSED_PASS: u32 = 2;
pub const LIGHT_BEAM_PASS: u32 = 3;
pub const LIGHT_SAMPLES_PASS: u32 = 4;

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

/// Whether a slot may keep the node `c` at `zoom`, the world's name of it (`cast::Node::idx`):
/// a zoom coarser than `FINEST_ZOOM`, and on each axis an index within twice the world's
/// span about it, which every node of zoom 1 or finer within `cast::REACH` of an origin in
/// the world has (pxl-render walk_node.wgsl `keeps`).
pub const fn keeps(zoom: Zoom, c: [i32; 3]) -> bool {
    if zoom >= 32 {
        return false;
    }
    const fn held(v: i32, zoom: Zoom) -> bool {
        let v = v >> zoom;
        v == 0 || v == -1
    }
    held(c[0], zoom) && held(c[1], zoom) && held(c[2], zoom)
}

/// pxl-render walk_node.wgsl `mix_hash`.
const fn mix_hash(h: u32, v: u32) -> u32 {
    (h ^ v).wrapping_mul(0x9E37_79B1).rotate_left(15)
}

/// The node `c` of `zoom`, the world's name of it, hashed (pxl-render walk_node.wgsl
/// `node_hash`).
pub const fn node_hash(zoom: Zoom, c: [i32; 3]) -> u32 {
    let mut h = zoom.wrapping_mul(0x85EB_CA77);
    h = mix_hash(h, c[0] as u32);
    h = mix_hash(h, c[1] as u32);
    h = mix_hash(h, c[2] as u32);
    h ^ h >> 16
}

/// The first slot of the bucket that keeps the node `c` of `zoom` (pxl-render walk_node.wgsl
/// `bucket_of`).
pub const fn bucket_of(zoom: Zoom, c: [i32; 3]) -> u32 {
    node_hash(zoom, c) & (ENTRIES - 1) & !(BUCKET - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::BOX_WHOLE as BOX;

    /// A cell kept by its node alone holds for every pair its query answers it its cell at, and
    /// no other; a split holds for its own pair alone; a claim holds for none.
    #[test]
    fn a_cell_key_holds_for_every_pair_that_stops_there() {
        let pair = |far: Zoom, near: Zoom| far | near << ZOOM_BITS;
        let cell = [VOID + 1, 0];
        let tag = PASSES * 7 + 1;
        for zoom in 0..=crate::FINEST_ZOOM {
            for near in 0..=crate::FINEST_ZOOM {
                for far in [0, near / 2, near] {
                    let asked = pair(far, near);
                    let kept = key_zooms(zoom, asked, Kind::Cell, cell);
                    let key = tag | kept << TAG_BITS;
                    let whole = zoom < near;
                    assert_eq!(kept & ((1 << ZOOM_BITS) - 1), CELL_FAR);
                    assert_eq!(kept >> ZOOM_BITS != 0, whole);
                    for n2 in 0..=crate::FINEST_ZOOM {
                        let other = pair(n2 / 2, n2);
                        assert_eq!(
                            holds_for(key, zoom, other),
                            whole || n2 <= zoom,
                            "{zoom} {near} {n2}"
                        );
                    }
                    let split = tag | key_zooms(zoom, asked, Kind::Split, BOX) << TAG_BITS;
                    assert!(holds_for(split, zoom, asked));
                    assert!(!holds_for(CLAIMED, zoom, asked));
                    assert!(!holds_for(CLAIM_FAR | 7, zoom, asked));
                }
            }
        }
    }

    use crate::cast::tests::reached;
    use crate::cast::Node;

    /// Every node within `REACH` of an origin in the world, at every zoom from 1 to the
    /// finest's coarser, is one a slot may keep, about X's seam and the world's edges alike; no node
    /// at the finest is.
    #[test]
    fn keeps_every_node_within_reach() {
        for (o, p) in (0..20_000u64).filter_map(reached) {
            for zoom in 1..=crate::FINEST_ZOOM {
                let keep = if zoom >= crate::FINEST_ZOOM {
                    u32::MAX
                } else {
                    !(u32::MAX >> zoom)
                };
                let n = Node {
                    corner: p.0.map(|c| c & keep),
                    zoom,
                    roots: p.1,
                };
                assert_eq!(
                    keeps(zoom, n.idx(o)),
                    zoom < crate::FINEST_ZOOM,
                    "{o:?} {p:?} {zoom}"
                );
            }
        }
    }
}
