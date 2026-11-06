// pxl-engine/record.rs

//! gpu-portable: an observer's record of voxels, kept across frames, the eye's and the
//! light's, one table each laid and kept alike (pxl-render record.wgsl, record_put.wgsl,
//! sample_box.wgsl, light_sample_box.wgsl, record_stops.wgsl, record_draw.wgsl,
//! light_record.wgsl, cast.wgsl, light.wgsl, face.wgsl; gpu/ray/raster.rs). A voxel is a
//! node the observer's rays stopped on, the World Query's answer for it and the faces rays
//! entered it across. The rays write it where they
//! stop; the raster draws every voxel the record holds, each frame, whatever the rays of that
//! frame reached; the record decides what is drawn
//! (doc/minutes/20261106-the-frame-is-what-it-must-draw.md).
//!
//! **The key** is the node, world-absolute: its zoom, its corner on the finest grid and the
//! root it lies in (`Key`), with the band its ray asked (the light's, `light::Light::band`;
//! the eye's 0). A stopped cell's
//! answer is a function of its key: the query answers a node it stops in by its cell, which
//! takes no asked zoom (pxl-render world_split.wgsl `world_node`), and a node coarser than
//! asked answers only void, which no ray stops in. So an entry stays the world's answer as
//! the eye moves, until the world changes under it.
//!
//! **The table:** `ENTRIES` slots in buckets of `BUCKET`, a key in its bucket alone
//! (`bucket_of`). A slot's first word says what it holds (`R_HELD`): free, claimed for a key
//! while a stop writes it (`claim_of`), or a key, written last, so a held word names words
//! already written.
//!
//! **The light's** is laid, written and drawn as the eye's, by the same code, over its own
//! table: its key's band the light's (`light::Light::band`), its frame the light's cast's
//! number, its samples its stops (sample_box.wgsl over sample.light.wgsl), its faces' words
//! asked none, as its slots read none (sample_box.light.wgsl), and its draw the light's
//! (record_draw.wgsl `light_record_draw_cs`): every held voxel whose box lies on the light's
//! box, in units from where box texel (0, 0)'s ray starts, fresh where its band is the band
//! of the tile its box's middle lies on (`light_fresh`), stale behind. Its slots take only a
//! voxel of their own band (light.wgsl), so a stale one is never taken, only kept from
//! hiding a fresh one. What it holds is no answer of the light's direction: a node, its
//! answer and the faces some light ray reached through air are the world's; the direction
//! only places their boxes. So a step of the sun keeps every entry and redraws the map off
//! them along the new direction, the tiles the cast claims (`light::claims`); what the new
//! direction meets that no light ray met yet is not in the record, and the epoch the step
//! begins casts the light's samples over every tile anew, nearest first within the budget,
//! so the map converges to the new direction's answer as the eye's picture does.
//!
//! **Its writers, in a frame's order:**
//! 1. Forget (pxl-render record_draw.wgsl `record_forget_cs`), the eye's and the light's, with
//!    the node cache's: every
//!    slot where it forgets all; else a slot whose node, or a node beside it across a face,
//!    the node cache's own rule forgets (forget.wgsl `forgets`, `over_box`). The faces'
//!    words read the cells beside a face, so a change beside the node forgets it.
//!    `reads_written` is not asked (**Edits**, below).
//! 2. Stops, each a node a ray stopped on with the face it entered, its answer and the line
//!    its face's word is asked off (pxl-render record_put.wgsl `upsert`), one way in for
//!    every ray: the samples, in two passes (sample_box.wgsl), the first the samples heading
//!    their node's run, the second every stopped sample; then the missed pixels' hits
//!    (record_stops.wgsl, `beam::HIT_ASSET`, `HIT_LINE`), after their walk. A stop finds the
//!    first slot of its bucket that holds its key; where none does, and it may claim, it
//!    claims one, a free one or the one least lately seen (`victim`), its claim word its
//!    key's (`claim_of`), taken by setting `CLAIM` on the word it read, which one lane alone
//!    sets where no claim holds the word, and where the word was not the one it read put
//!    off again, no other store reaching a held word while a pass claims (Safari compiles
//!    no compare-exchange); it reads its bucket again, and writes it unless another slot holds
//!    the key or the same claim stands before it in the bucket, else lets it go free. A
//!    stop that meets its key's claim waits on no other lane: it is put off to a pass past a
//!    dispatch's end, when every claim of the pass before is written or let go and the key
//!    is held, and taken there without claiming: a sample's by the samples' second pass,
//!    which takes every sample; a hit's by the deferred hits' pass (record_stops.wgsl
//!    `record_deferred_cs`, `beam::Grid::defers_at`, `HITS` at most). A stop that found or
//!    wrote its entry marks it seen and sets the face it entered across, asking its word the
//!    first time (face.wgsl, which writes `R_FACES`).
//! 3. The draw (record_draw.wgsl `record_draw_cs`) reads every held slot and writes the
//!    raster's boxes, a slot a key: a held slot whose key a slot before it in its bucket
//!    holds, two claims that each missed the other under WGSL's relaxed atomics, gives that
//!    one its faces and their words and goes free, drawn by it. No pass that claims runs
//!    with the draw, and every ask of the frame before was answered, so no word left to
//!    write names it. The raster draws them; `cast_cs` reads a pixel's slot and marks it
//!    seen. Each word has one writer a step.
//!
//! **Fresh or stale:** an entry is fresh where its node is the one the walk stops on from
//! this eye (`fresh`): it answers a cell at its zoom (its zoom at or past the zoom asked at
//! its nearest point, or it stopped whole) and its parent does not. A node's ancestor is
//! nearer at its nearest point, so asked no coarser, and the parent's test holds for every
//! ancestor: two fresh entries never nest. A stale entry is drawn behind every fresh one
//! (`raster::STALE`) and is never a pixel's answer, as the light's stale one is never taken:
//! where no fresh voxel covers a pixel, the pixel is a hole that shows what the record last
//! found there until its walk ends (pxl-render cast.wgsl `cast_cs`), and the walk's stop
//! writes the fresh voxel, after which the stale one goes unseen and is evicted (`victim`).
//! Where a fresh voxel covers the pixel, the pixel takes the fresh one, so no second surface.
//! A stale box about the eye (`raster::is_eyed`) is not drawn: drawn over the whole screen
//! at the nearest stale depth, it would stand before every other stale box and show at
//! every pixel no fresh box covers, where it lies at none of them.
//! A stale node at the zoom now asked is another key, written where a sample or a hole's walk
//! finds it.
//!
//! **Edits:** every write of what is placed that changes the tree or a record names the
//! cube it wrote (pxl-kernel domain/layer/object/store.rs `Store::write`, `set_fold`,
//! `clear_fold`), handed the GPU as its leaf cells' columns, joined past their room, never
//! dropped (domain/scene/decor/gpu.rs `note_edited`; gpu/ray/cast.rs `hold`); a write that
//! names none, or child groups past their room, forgets all. A cell's answer reads the
//! tree along its own path alone, which a write changes only within the cube and over it,
//! both met by `over_box` on the cell's column. `reads_written` forgets what was handed a
//! node that reads a rewritten child group, a slot that may now hold another node's words:
//! the walks' state, which the record does not keep.
//!
//! **Validity is the node cache's, eviction is not:** a world edit invalidates an entry by
//! the node cache's own predicates, in its pass (1. above); the record keeps its own table
//! and evicts by its own rule alone (below), never because the node cache let a slot go.
//!
//! **The faces drawn** are every face, a side of six (`ray::face`), some ray, from any
//! eye or any light, entered the node through: each such entry proves that side borders air
//! at the node's zoom, a world fact until an edit forgets the entry, so it is drawn from any
//! eye where it is turned to it, and the depth test decides the rest. A side no ray entered
//! is not drawn, the other side of an entered one's axis among them, which may lie against
//! what fills: an eye that moves past the node's plane, or a sun that turns across it, sees
//! that side only once a ray reaches it, a hole until then, the rays' work. What lies about
//! a face is asked a word a side.
//!
//! **A recorded voxel hides no nearer one from the rays:** the samples walk from their cells'
//! starts, which the beam rays find from the eye, not from the record, so a sample stops on
//! the first cell along its ray whatever the record holds there; an occluder a sample's ray
//! meets is written the frame it is cast. While the view and the world stand, the samples
//! are cast again at each offset of the rolled lattice (`beam::roll`), one a frame. The
//! offsets together lay a regular lattice `grid::CELL`/16 apart, so a nearest occluder whose
//! projection holds a square of that side aligned to it, or any disc √2 · `CELL`/16 across,
//! holds a sample of some offset and is written within `beam::ROLLS` standing frames of the
//! last change.
//!
//! **Eviction:** a sample that finds its bucket full takes the slot seen least lately, never
//! one seen this frame or the last (`victim`), so no voxel a pixel took last frame goes.
//! Where every slot was, it writes none, and its pixels walk.

use crate::cast::{self, Detail, Node, Spacing};
use crate::light::Light;
use crate::{Zoom, WORKGROUPS_MOST};

/// The head: the frame's number, the node cache's (`node_cache::FRAME`), which an entry is
/// marked seen with.
pub const FRAME: u32 = 0;
pub const HEAD_WORDS: u32 = 4;

/// The slots, and a bucket's: a power of two each, so a hash masks to a bucket.
pub const ENTRIES: u32 = 1 << 19;
pub const BUCKET: u32 = 8;

/// The threads of a workgroup of the passes over every slot (pxl-render record_draw.wgsl).
pub const DRAW_LANES: u32 = 64;
const _: () = assert!(
    ENTRIES.is_multiple_of(DRAW_LANES) && ENTRIES / DRAW_LANES <= WORKGROUPS_MOST,
    "the passes over every slot take each once in one dimension of one dispatch"
);

/// An entry's words: what it holds (`held`); its node's corner and its root, X, Y, Z,
/// world-absolute (`Key`); the answer's asset; its state (`FACES`, `LACKS`, `WHOLE`); the
/// last frame a sample stopped on it or a pixel took it; what lies about each face drawn, a
/// word a side, as `ray::face` numbers them (pxl-engine `face::word`).
pub const R_HELD: u32 = 0;
pub const R_CORNER: u32 = 1;
pub const R_ROOTS: u32 = 4;
pub const R_ASSET: u32 = 7;
pub const R_STATE: u32 = 9;
pub const R_SEEN: u32 = 10;
pub const R_FACES: u32 = 11;
pub const ENTRY_WORDS: u32 = 17;

/// The buffer's words, whatever is drawn.
pub const WORDS: u32 = HEAD_WORDS + ENTRIES * ENTRY_WORDS;
const _: () = assert!(
    (WORDS as u64) * 4 <= 134_217_728,
    "the record binds whole as storage on every device"
);

/// The first word of slot `slot`'s entry.
pub const fn entry_at(slot: u32) -> u32 {
    HEAD_WORDS + slot * ENTRY_WORDS
}

/// `R_HELD` of a free slot; a claim's bit, under which its key's hash lies (`claim_of`); and
/// a key's bit, under which its band and zoom lie (`held`).
pub const FREE: u32 = 0;
pub const CLAIM: u32 = 1 << 30;
pub const HELD: u32 = 1 << 31;

const _: () = assert!(
    crate::FINEST_ZOOM < 1 << 8,
    "a zoom fits under a held word's band"
);

/// The state's bits: a face drawn a side (`raster::faces`), whether the node lacks, and
/// whether its ray stopped on it coarser than it asked, a cell the query answers whole.
pub const FACES: u32 = (1 << 6) - 1;
pub const LACKS: u32 = 1 << 6;
pub const WHOLE: u32 = 1 << 7;

/// Where an ask is the record's (`beam::ASK_TO`): the bit set on the word face.wgsl leaves
/// the face's word in, among the record's words, not the beam rays'; its node then the
/// record's entry (`beam::ASK_NODE`, `entry_at`).
pub const TO_RECORD: u32 = 1 << 31;
const _: () = assert!(WORDS < TO_RECORD, "an ask names any word of the record");

/// A node as the record keys it: its zoom, its corner on the finest grid and the root it
/// lies in, world-absolute, and the band its ray asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub zoom: Zoom,
    pub corner: [u32; 3],
    pub roots: [i32; 3],
    pub band: u32,
}

impl Key {
    /// The node `n` as a walk from the origin `o` holds it, at `band`.
    pub fn of(o: [i32; 3], n: &Node, band: u32) -> Key {
        let r0 = cast::roots_of(o);
        Key {
            zoom: n.zoom,
            corner: n.corner,
            roots: [0, 1, 2].map(|a| r0[a] + n.roots[a]),
            band,
        }
    }

    /// The node as a walk from the origin `o` holds it.
    pub fn node(&self, o: [i32; 3]) -> Node {
        let r0 = cast::roots_of(o);
        Node {
            corner: self.corner,
            zoom: self.zoom,
            roots: [0, 1, 2].map(|a| self.roots[a] - r0[a]),
        }
    }

    /// Its held word (`R_HELD`).
    pub const fn held(&self) -> u32 {
        HELD | self.band << 8 | self.zoom
    }
}

/// The zoom and the band a held word names.
pub const fn zoom_of(held: u32) -> Zoom {
    held & 0xff
}

pub const fn band_of(held: u32) -> u32 {
    (held & !HELD) >> 8
}

/// pxl-render walk_node.wgsl `mix_hash`.
const fn mix(h: u32, v: u32) -> u32 {
    (h ^ v).wrapping_mul(0x9E37_79B1).rotate_left(15)
}

/// The first slot of the key's bucket.
pub const fn bucket_of(k: &Key) -> u32 {
    let mut h = k.held().wrapping_mul(0x85EB_CA77);
    h = mix(h, k.corner[0]);
    h = mix(h, k.corner[1]);
    h = mix(h, k.corner[2]);
    h = mix(h, k.roots[0] as u32);
    h = mix(h, k.roots[1] as u32);
    h = mix(h, k.roots[2] as u32);
    (h ^ h >> 16) & (ENTRIES - 1) & !(BUCKET - 1)
}

/// The claim word of a slot being written for `k` (`R_HELD`): `CLAIM` and its key's hash,
/// so a stop tells a claim of its own key from another's; two keys of one hash in one bucket
/// take one slot between them this time. The hash is never 0: `CLAIM` alone is a free slot
/// a claim is taking (pxl-render record_put.wgsl `claim`), no key's claim.
pub const fn claim_of(k: &Key) -> u32 {
    let mut h = k.held().wrapping_mul(0x27D4_EB2F);
    h = mix(h, k.corner[0]);
    h = mix(h, k.corner[1]);
    h = mix(h, k.corner[2]);
    h = mix(h, k.roots[0] as u32);
    h = mix(h, k.roots[1] as u32);
    h = mix(h, k.roots[2] as u32);
    let hash = (h ^ h >> 16) & (CLAIM - 1);
    CLAIM | if hash == 0 { 1 } else { hash }
}

/// The zoom the walk asks the node `n` at its nearest point from the origin `o`, the eye's
/// zooms `range` (`cast::node_zooms`).
fn near(o: [i32; 3], n: &Node, range: [Zoom; 2]) -> Zoom {
    cast::node_zooms(
        o,
        n,
        Detail {
            range,
            spacing: Spacing::NONE,
        },
    )[1]
}

/// Whether a light's ray that stopped on the node of key `k` stopped on it whole: coarser
/// than its band, the one zoom it asks (`light::Light::band`).
pub const fn light_whole(k: &Key) -> bool {
    k.zoom < k.band
}

/// Whether the light's entry of key `k` is fresh for the light `l`, its map `size` a side,
/// the eye's zooms `range`: its band is the band of the tile its box's middle `mid` lies on,
/// units from the eye (`light::Light::tile_of`, `light::Light::band`), so the slots about it
/// ask it at its band. A light's slot takes only a voxel of its own band (pxl-render
/// light.wgsl), so a stale one is drawn behind every fresh one (`raster::STALE`).
pub fn light_fresh(l: &Light, size: u32, range: [Zoom; 2], k: &Key, mid: [f32; 3]) -> bool {
    l.band(size, range, l.tile_of(mid)) == k.band
}

/// Whether a ray from the origin `o` that stopped on the node `n` stopped on it whole:
/// coarser than the zoom it asked there, so the query answered it a cell for the cell's own
/// sake (pxl-render world_split.wgsl `world_node`'s `whole`), not for the zoom.
pub fn stopped_whole(o: [i32; 3], n: &Node, range: [Zoom; 2]) -> bool {
    n.zoom < near(o, n, range)
}

/// Whether the entry of key `k`, `whole` where it stopped whole (`WHOLE`), is the node the
/// walk stops on from the origin `o`, the eye's zooms `range`: it answers a cell there and
/// its parent does not.
pub fn fresh(o: [i32; 3], k: &Key, whole: bool, range: [Zoom; 2]) -> bool {
    let n = k.node(o);
    if !whole && n.zoom < near(o, &n, range) {
        return false;
    }
    if n.zoom == 0 {
        return true;
    }
    let keep = !(u32::MAX >> (n.zoom - 1));
    let parent = Node {
        corner: n.corner.map(|c| c & keep),
        zoom: n.zoom - 1,
        roots: n.roots,
    };
    parent.zoom < near(o, &parent, range)
}

/// The share the screen's tangents are grown by where the draw asks whether a box is in
/// view (`in_view`), for the raster's jitter and growth (`raster::GROWTH`).
pub const VIEW_GROWTH: f32 = 1.0 / 64.0;

/// Whether the box `lo`..`hi`, units from the eye, meets what the eye of axes `right`, `up`
/// and `forward` sees, its screen's tangents `tan` grown by `VIEW_GROWTH`, as far as `far`:
/// some point of it before the eye, its nearest along `forward` short of `far`, and some
/// point within each of the screen's four sides (pxl-render record_draw.wgsl `in_view`).
pub fn in_view(
    (lo, hi): ([f32; 3], [f32; 3]),
    [right, up, forward]: [[f32; 3]; 3],
    tan: [f32; 2],
    far: f32,
) -> bool {
    // The most of `n · p` over the box.
    let reaches = |n: [f32; 3]| -> f32 {
        let m = |a: usize| {
            let (x, y) = (n[a] * lo[a], n[a] * hi[a]);
            if x > y {
                x
            } else {
                y
            }
        };
        m(0) + m(1) + m(2)
    };
    let t = [tan[0] * (1.0 + VIEW_GROWTH), tan[1] * (1.0 + VIEW_GROWTH)];
    let side = |k: f32, s: [f32; 3], sign: f32| {
        [
            k * forward[0] + sign * s[0],
            k * forward[1] + sign * s[1],
            k * forward[2] + sign * s[2],
        ]
    };
    reaches(forward) > 0.0
        && -reaches(forward.map(|v| -v)) < far
        && reaches(side(t[0], right, -1.0)) >= 0.0
        && reaches(side(t[0], right, 1.0)) >= 0.0
        && reaches(side(t[1], up, -1.0)) >= 0.0
        && reaches(side(t[1], up, 1.0)) >= 0.0
}

/// The slot of a bucket a sample takes for a key no slot holds, its slots' held words and
/// last seen frames in order, in frame `frame`: the first free; else the one seen least
/// lately, the first of those, where it was seen before the frame before this; else none.
pub fn victim(bucket: &[(u32, u32)], frame: u32) -> Option<usize> {
    if let Some(k) = bucket.iter().position(|&(h, _)| h == FREE) {
        return Some(k);
    }
    let mut best: Option<(usize, u32)> = None;
    for (k, &(h, seen)) in bucket.iter().enumerate() {
        if h & HELD == 0 || seen.saturating_add(2) > frame {
            continue;
        }
        if best.is_none_or(|(_, s)| seen < s) {
            best = Some((k, seen));
        }
    }
    best.map(|(k, _)| k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FINEST_ZOOM, UNIT_ZOOM};

    fn node(zoom: Zoom, corner: [u32; 3], roots: [i32; 3]) -> Node {
        let keep = if zoom >= FINEST_ZOOM {
            u32::MAX
        } else {
            !(u32::MAX >> zoom)
        };
        Node {
            corner: corner.map(|c| c & keep),
            zoom,
            roots,
        }
    }

    fn hash(v: u64) -> u64 {
        let mut z = v.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ z >> 30).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ z >> 27).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ z >> 31
    }

    /// A key names its node again from any origin, and a held word its zoom and band.
    #[test]
    fn key_names_its_node() {
        for i in 0..4096u64 {
            let h = hash(i);
            let o = [h as i32, (h >> 20) as i32 >> 8, (h >> 40) as i32 >> 12];
            let o2 = [o[0].wrapping_add(1 << 30), o[1], o[2] ^ 0x1234];
            let n = node(
                (h % u64::from(FINEST_ZOOM + 1)) as Zoom,
                [h as u32, (h >> 13) as u32, (h >> 29) as u32],
                [0, 0, -((h >> 7) as i32 & 1)],
            );
            let k = Key::of(o, &n, (h >> 50) as u32 & 0x3f);
            assert_eq!(k.node(o), n);
            assert_eq!(Key::of(o2, &k.node(o2), k.band), k);
            assert_eq!(zoom_of(k.held()), n.zoom);
            assert_eq!(band_of(k.held()), k.band);
            assert_ne!(k.held() & HELD, 0);
            assert_eq!(k.held() & CLAIM, 0);
            assert_eq!(bucket_of(&k) % BUCKET, 0);
            assert_eq!(claim_of(&k) & (HELD | CLAIM), CLAIM);
            assert_ne!(claim_of(&k), CLAIM);
            assert!(bucket_of(&k) < ENTRIES);
        }
    }

    /// Two fresh entries never nest: from any origin, of a node and every node over it, at
    /// most one is fresh, and the node a walk would stop on along a column is fresh.
    #[test]
    fn fresh_never_nests() {
        let range = [4, FINEST_ZOOM];
        for i in 0..2048u64 {
            let h = hash(i);
            let o = [
                (h as i32) >> 4,
                (h >> 16) as i32 >> 10,
                ((h >> 40) as i32 >> 18) + (1 << 12),
            ];
            let leaf = node(
                FINEST_ZOOM,
                [
                    (o[0] as u32).wrapping_add((h >> 3) as u32 % (1 << 20)),
                    (o[1] as u32).wrapping_add(cast::Y_START) ^ ((h >> 9) as u32 % (1 << 22)),
                    (o[2] as u32).wrapping_add(cast::Z_START),
                ],
                [0; 3],
            );
            let mut first = None;
            let mut cnt = 0;
            for z in 0..=FINEST_ZOOM {
                let n = node(z, leaf.corner, leaf.roots);
                let k = Key::of(o, &n, 0);
                if fresh(o, &k, false, range) {
                    first = first.or(Some(z));
                    cnt += 1;
                }
            }
            assert!(cnt <= 1, "{o:?}: fresh at {cnt} zooms");
            // The walk stops at the first zoom at or past its nearest point's: that one.
            let stops = (0..=FINEST_ZOOM).find(|&z| {
                let n = node(z, leaf.corner, leaf.roots);
                !stopped_whole(o, &n, range)
            });
            assert_eq!(first, stops, "{o:?}");
        }
    }

    /// A node that stopped whole is fresh whatever its zoom asks, where its parent is not.
    #[test]
    fn whole_is_fresh_where_its_parent_splits() {
        let o = [0, 0, 1 << 12];
        let n = node(
            UNIT_ZOOM - 2,
            [0, cast::Y_START, cast::Z_START + (1 << 12)],
            [0; 3],
        );
        let k = Key::of(o, &n, 0);
        let range = [0, FINEST_ZOOM];
        assert!(stopped_whole(o, &n, range));
        assert!(!fresh(o, &k, false, range));
        assert!(fresh(o, &k, true, range));
    }

    /// The first free slot; else the least lately seen of those seen before the frame
    /// before; else none.
    #[test]
    fn victim_spares_the_seen() {
        let held = HELD | 7;
        assert_eq!(victim(&[(held, 1), (FREE, 0), (FREE, 0)], 9), Some(1));
        assert_eq!(victim(&[(held, 5), (held, 3), (held, 4)], 9), Some(1));
        assert_eq!(victim(&[(held, 9), (held, 8), (held, 7)], 9), Some(2));
        assert_eq!(victim(&[(held, 9), (held, 8)], 9), None);
        assert_eq!(victim(&[(CLAIM | 3, 0), (held, 8)], 9), None);
    }
}
