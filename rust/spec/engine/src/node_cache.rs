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
//!   clip levels' entries that moved (`HEAD_WORDS`);
//! - the frame the eye's draw last ran (`DRAWN`);
//! - the eye's rolled lattice (`ROLL_K`);
//! - the entries, a slot each (`ENTRIES_AT`).
//!
//! That is all of it (`WORDS`). The pixels a frame's cast missed are
//! marked past the camera grid's words, a bit a pixel (`ray::holes_words`).
//!
//! An entry is its key, which says it is held — the walk that wrote it, by
//! its frame and its pass, and the zooms it keeps the node at (`key_zooms`) — then the
//! answer's kind and asset, what was handed the node after it was asked
//! (`world::Inheritance::words`), and the node, its zoom and its index, which tell
//! it from another node of the same slot (pxl-render forget.wgsl reads its column); then
//! what draws it, none until a ray stops on it (`W_STATE`, `W_SEEN`, `W_FACES`), written by
//! the stops, the draws and the faces' pass, and set to none where the slot is written.
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
//! kept already but not whole, sets it whole (ask.wgsl `cell_kept`); its stamp takes no
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
use crate::overwrite::BOXES;
use crate::world::INHERITANCE_WORDS;
use crate::{Zoom, ZOOM_BITS};

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
pub const LAST_TAKEN: u32 = HITS_CNT + 1;
/// The holes' work so far in the frame, both observers' queues', the nodes visited and those
/// asked: added to through atomics by each queue's asks, the visits its walk over the node
/// cache before them left in its words folded in (pxl-render queue_ask.wgsl, `ray::QUEUE_VISITED`),
/// and by its last walk; read by every walk's budget against its run's caps (`caps_at`).
/// Set to none by the host each frame a queue runs.
pub const VISITED: u32 = LAST_TAKEN + 1;
pub const ASKED: u32 = VISITED + 1;
/// The eye's samples and beam rays its lattice's runs left parked this frame, the budget not
/// reaching them or their cells' starts waiting on a beam ray (pxl-render samples.wgsl,
/// corners.wgsl `unreached`), which the cast's close reads (roll.wgsl, `ROLL_DONE`). Set to
/// none with the frame's work.
pub const LATTICE_LEFT: u32 = ASKED + 1;
/// Each observer's work its last cast or queue leaves owed, `count::OBSERVERS` words (`owed_at`):
/// the eye's, its holes its run left owed, an owed bit or a ray parked in the run's pool
/// (pxl-render parked_tiles.wgsl);
/// the light's, the tiles it wanted and left unclaimed, copied in from its args after the claim
/// (`light::ARGS_LEFT`), and the slots its last walk left parked, which add to it. None where
/// nothing is owed. Kept apart each frame the observer's queue runs, as a later frame's head
/// written clears it, and read back from there (`count::OWED`).
pub const OWED_AT: u32 = LATTICE_LEFT + 1;
/// Each kind of run's caps it counts the frame's work to, the visits' and the asks'
/// (`ray::caps`), two words a kind (`caps_at`): the host's each frame a run of it runs.
pub const CAPS_AT: u32 = OWED_AT + count::OBSERVERS;
pub const HEAD_WORDS: u32 = CAPS_AT + 2 * count::RUNS;
/// The frame the eye's draw last ran, written by it (pxl-render record_draw.wgsl
/// `record_draw_cs`): the boxes the raster draws until it runs again name the entries it
/// marked seen then (`record::drawn`). Past the head the host writes each frame.
pub const DRAWN: u32 = HEAD_WORDS;
/// The eye's rolled lattice (`beam::ROLLS`), kept from cast to cast while its grid stands and
/// set to none by the host where the grid is laid anew: the offset this cast casts its samples
/// at, an index of `beam::roll` (`ROLL_K`); the offsets counted, a bit each, all of
/// `beam::ROLLS_DONE` once every one is (`ROLL_DONE`); whether the next cast's samples go
/// on from where the budget parked them rather than afresh (`ROLL_RESUMES`); and that offset,
/// across and down, f32 bits (`ROLL_AT`), which a sample's ray reads as a word as it read the
/// host's before, so its tangents round as the CPU's `beam::Grid::sample` rounds them; and the
/// casts since the grid was laid (`ROLL_CASTS`), past the first of which the eye's beam rays
/// a cast left parked go on rather than start afresh (pxl-render beam.eye.wgsl
/// `beams_resume`). Written by the close of the frames the eye casts its samples in alone
/// (pxl-render roll.wgsl `roll_cs`), which counts the offset where the lattice's runs left
/// none parked (`LATTICE_LEFT`) and takes the next.
pub const ROLL_K: u32 = DRAWN + 1;
pub const ROLL_DONE: u32 = ROLL_K + 1;
pub const ROLL_RESUMES: u32 = ROLL_K + 2;
pub const ROLL_AT: u32 = ROLL_K + 3;
pub const ROLL_CASTS: u32 = ROLL_K + 5;
pub const ROLL_WORDS: u32 = 6;

/// The first of run kind `run`'s caps (`CAPS_AT`, `count::RUNS`).
pub const fn caps_at(run: u32) -> u32 {
    CAPS_AT + 2 * run
}

/// The word of `observer`'s owed work (`OWED_AT`).
pub const fn owed_at(observer: u32) -> u32 {
    OWED_AT + observer
}

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
/// What draws it (`record`): its state, the faces rays entered and asked about and whether
/// some ray stopped on it whole (`record::FACES`, `record::ASKED`, `record::WHOLE`); the last
/// frame a draw drew it (`record::drawn`); and what lies about each face, a word a side, as
/// `ray::face` numbers them (`face::word`).
pub const W_STATE: u32 = W_NODE + 4;
pub const W_SEEN: u32 = W_STATE + 1;
pub const W_FACES: u32 = W_SEEN + 1;
pub const ENTRY_WORDS: u32 = W_FACES + 6;

/// The slots, a power of two, so a key's hash masks to one: what the table must hold, derived
/// in the record that draws its entries (`record::entries_for`, which asserts it).
pub const ENTRIES: u32 = 1 << 20;
pub const ENTRIES_AT: u32 = ROLL_K + ROLL_WORDS;
/// The buffer's words, whatever is drawn: none of them a pixel's.
pub const WORDS: u32 = ENTRIES_AT + ENTRIES * ENTRY_WORDS;
const _: () = assert!(
    (WORDS as u64) * 4 <= 134_217_728,
    "the node cache binds whole as storage on every device"
);

/// The first word of slot `slot`'s entry.
pub const fn entry_at(slot: u32) -> u32 {
    ENTRIES_AT + slot * ENTRY_WORDS
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

    use crate::cast::{point_at, Node, Ray, REACH, Z_START};
    use crate::{FRAC_W, HEIGHT_BITS};

    /// Every node within `REACH` of an origin in the world, at every zoom from 1 to the
    /// finest's coarser, is one a slot may keep, about X's seam and the world's edges alike; no node
    /// at the finest is.
    #[test]
    fn keeps_every_node_within_reach() {
        let hash = |v: u64| {
            let mut z = v.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ z >> 30).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ z >> 27).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ z >> 31
        };
        let height = 1i64 << (HEIGHT_BITS + FRAC_W);
        for i in 0..20_000u64 {
            let h = hash(i);
            let x = [0, i32::MAX, i32::MIN, (h >> 7) as i32][(h & 3) as usize]
                .wrapping_add((h >> 9) as i32 >> 12);
            let y = [i32::MAX, i32::MIN, (h >> 21) as i32][(h >> 3) as usize % 3]
                .wrapping_sub((h >> 40) as i32 >> 14);
            // In the world's height, or at its floor or its top.
            let z = match (h >> 5) % 3 {
                0 => -(Z_START as i32),
                1 => (height - i64::from(Z_START) - 1) as i32,
                _ => ((h >> 24) as i64 % height - i64::from(Z_START)) as i32,
            };
            let o = [x, y, z];
            let g = hash(i ^ 0xA5A5);
            let d = [g, g >> 21, g >> 42].map(|v| (v & 0x1f_ffff) as f32 / 1_048_576.0 - 1.0);
            let len = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
            if len < 1e-3 {
                continue;
            }
            let ray = Ray::new([d[0] / len, d[1] / len, d[2] / len], REACH as f32);
            let t = (g >> 50) as f32 / (1u64 << 14) as f32 * (REACH - 1) as f32;
            let p = point_at(o, &ray, t);
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
