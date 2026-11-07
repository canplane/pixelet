// pxl-engine/record.rs

//! gpu-portable: what draws the node cache's entries, the observers' record of voxels, kept
//! across frames in the node cache's own slots (`node_cache::STATES_AT`; pxl-render
//! record.wgsl, record_put.wgsl, sample_box.wgsl, record_stops.wgsl, record_draw.wgsl,
//! cast.wgsl, light.wgsl, face.wgsl, forget.wgsl, ask.wgsl; gpu/ray/raster.rs). A voxel is an
//! entry of a node some ray stopped on: the World Query's answer for it, the node cache's,
//! and the faces rays entered it across. The rays' stops write the faces where they stop;
//! the raster draws every voxel, each frame, whatever the rays of that frame reached; the
//! node cache's drawable entries decide what is drawn
//! (doc/minutes/20261106-the-frame-is-what-it-must-draw.md).
//!
//! **The node:** an entry is the node cache's, keyed by the node and the zooms a walk asked
//! it at. A stopped cell's answer takes no zooms asked: the query answers a node it stops in
//! by its cell, which takes no asked zoom (pxl-render world_split.wgsl `world_node`;
//! world.test.rs `cell_answer_takes_no_zooms`), and a node coarser than asked answers only
//! void, which no ray stops in. So a node's entries at several zooms hold one answer, and one
//! of them draws it: a stop takes the entry of its node that answers its cell and already
//! draws, else the first that answers it (pxl-render record.wgsl `find`), and no pass that
//! stops writes a key, so every stop of a node in a pass takes the one entry, and a second
//! entry of the node draws only once the first has left the table. An entry stays the
//! world's answer as the eye moves and the light's bands change, until the world changes
//! under it. Whether a ray stopped on it coarser than it asked (`WHOLE`) is the world's too:
//! the query answers a node coarser than asked a cell only where nothing finer is in it
//! (`world::node`'s `whole`), so any ray's stop that finds it so sets it, the eye's or the
//! light's.
//!
//! **The light** writes and draws the same entries as the eye, by the same code: its samples
//! its stops (sample_box.wgsl over sample.light.wgsl), whole where coarser than the band its
//! ray asked (`light::Light::band`, `light_whole`), the faces it enters set (`FACES`), their
//! words asked none, as its slots read none (sample_box.light.wgsl), and its draw the light's
//! (record_draw.wgsl `light_record_draw_cs`): every voxel whose box lies on the light's box,
//! in units from where box texel (0, 0)'s ray starts, fresh where it is the node a ray asking
//! the band of the tile its box's middle lies on stops on (`light_fresh`), stale behind. Its
//! slots take only a voxel fresh at their own band (light.wgsl), so a stale one is never
//! taken, only kept from hiding a fresh one. What it holds is no answer of the light's
//! direction: a node, its answer and the faces some light ray reached through air are the
//! world's; the direction only places their boxes. So a step of the sun keeps every entry and
//! redraws the map off them along the new direction, the tiles the cast claims
//! (`light::claims`); what the new direction meets that no light ray met yet is not drawn,
//! and the epoch the step begins casts the light's samples over every tile anew, nearest
//! first within the budget, so the map converges to the new direction's answer as the eye's
//! picture does.
//!
//! **Its writers, in a frame's order:**
//! 1. Forget, the node cache's (pxl-render forget.wgsl `forget_cs`): every slot where it
//!    forgets all; else a slot whose node its rule forgets (`forgets`, `over_box`,
//!    `reads_written`), answer and all; else, where a node beside a drawable entry across a
//!    face is forgotten so, what draws it, its faces and their words, which read the cells
//!    beside a face, its answer kept.
//! 2. The walks, which write an entry's answer and set what draws it to none
//!    (pxl-render ask.wgsl `fill`).
//! 3. Stops, each a node a ray stopped on with the face it entered, its answer and the line
//!    its face's word is asked off (pxl-render record_put.wgsl `observe_stop`), one way in for
//!    every ray: the samples (sample_box.wgsl), then the missed pixels' hits
//!    (record_stops.wgsl, `beam::HIT_ASSET`, `HIT_LINE`), after their walk. A stop finds its
//!    node's entry (above) and sets the face it entered across; an observer that reads faces'
//!    words, the eye, asks its word the first time it enters it (`ASKED`; face.wgsl, which
//!    writes `node_cache::R_FACES`). A stop whose node no slot keeps sets none, and its pixel
//!    walks again.
//! 4. The draws (record_draw.wgsl `record_draw_cs`, `light_record_draw_cs`) read every slot
//!    and write a box for each drawable entry, and mark it drawn (`node_cache::R_DRAWN`); the
//!    eye's draw names the frame it ran (`node_cache::DRAWN`). The raster draws them;
//!    `cast_cs` and light.wgsl read a pixel's slot. Each word has one writer a step.
//!
//! **Fresh or stale:** an entry is fresh where its node is the one the walk stops on from
//! this eye (`fresh`): it answers a cell at its zoom (its zoom at or past the zoom asked at
//! its nearest point, or it stopped whole) and its parent does not. A node's ancestor is
//! nearer at its nearest point, so asked no coarser, and the parent's test holds for every
//! ancestor: two fresh entries never nest. A stale entry is drawn behind every fresh one
//! (`raster::STALE`) and is never a pixel's answer, as the light's stale one is never taken:
//! where no fresh voxel covers a pixel, the pixel is a hole that shows what was last found
//! there until its walk ends (pxl-render cast.wgsl `cast_cs`), and the walk's stop draws the
//! fresh voxel, after which the stale one goes undrawn and is evicted (`drawn`). Where a
//! fresh voxel covers the pixel, the pixel takes the fresh one, so no second surface. A stale
//! voxel more than a zoom coarser than the eye asks where it lies is not drawn: a voxel of a far
//! view seen from near stands as a wall no fresh voxel covers where the ray goes on to the sky,
//! and goes undrawn so it is evicted. A stale
//! box about the eye (`raster::is_eyed`) is not drawn: drawn over the whole screen at the
//! nearest stale depth, it would stand before every other stale box and show at every pixel
//! no fresh box covers, where it lies at none of them. A stale node at the zoom now asked is
//! another node, drawn where a sample or a hole's walk stops on it.
//!
//! **Edits:** every write of what is placed that changes the tree or a record names the
//! cube it wrote (pxl-kernel domain/layer/object/store.rs `Store::write`, `set_fold`,
//! `clear_fold`), handed the GPU as its leaf cells' columns, joined past their room, never
//! dropped (domain/scene/decor/gpu.rs `note_edited`; gpu/ray/cast.rs `hold`); a write that
//! names none, or child groups past their room, forgets all. A cell's answer reads the
//! tree along its own path alone, which a write changes only within the cube and over it,
//! both met by `over_box` on the cell's column; `reads_written` forgets an entry handed a
//! slot of a rewritten child group, a drawable one with the rest: one rule forgets.
//!
//! **The faces drawn** are faces, a side of six (`ray::face`), some ray entered the node
//! through: each such entry proves that side borders air at the node's zoom, a world fact
//! until an edit forgets the entry, so it is drawn from any eye where it is turned to it, and
//! the depth test decides the rest. The light draws every face any ray entered (`FACES`); the
//! eye, which shows what lies about a face, those an eye's ray entered and asked that of
//! (`ASKED`), so a face only the light entered is a hole to the eye until an eye's ray reaches
//! it and asks it. A side no ray entered is not drawn, the other side of an entered one's
//! axis among them, which may lie against what fills: an eye that moves past the node's
//! plane, or a sun that turns across it, sees that side only once a ray reaches it, a hole
//! until then, the rays' work. What lies about a face is asked a word a side.
//!
//! **A drawn voxel hides no nearer one from the rays:** the samples walk from their cells'
//! starts, which the beam rays find from the eye, not from what is drawn, so a sample stops on
//! the first cell along its ray whatever is drawn there; an occluder a sample's ray meets is
//! drawn the frame it is cast, and a pixel the samples miss is walked and stopped that frame.
//!
//! **Eviction** is the node cache's (`node_cache`), but never of an entry drawn in the last
//! two frames (`drawn`): a draw marks each entry it draws with its frame, a stop that makes an
//! entry drawable marks it with its own, and the boxes the raster keeps over frames without a
//! draw are the eye's last draw's, whose frame the node cache's head holds
//! (`node_cache::DRAWN`). Where a bucket holds none it may take, the walk keeps none, and its
//! pixels walk again.

use crate::cast::{self, Detail, Node, Spacing};
use crate::grid;
use crate::light::{Light, BEAM, TILE};
use crate::node_cache::{self, ENTRIES};

use crate::{Zoom, WORKGROUPS_MOST};

/// The threads of a workgroup of the passes over every slot (pxl-render record_draw.wgsl).
pub const DRAW_LANES: u32 = 64;
const _: () = assert!(
    ENTRIES.is_multiple_of(DRAW_LANES) && ENTRIES / DRAW_LANES <= WORKGROUPS_MOST,
    "the passes over every slot take each once in one dimension of one dispatch"
);

/// The node cache's slots (`node_cache::ENTRIES`): what the table must hold, over its load
/// (`LOAD_PER_8` eighths of its slots), rounded up to a power of two (`entries_for`).
///
/// What it holds: every voxel a view can draw, the eye's and the light's, and the nodes a
/// frame's asks keep. A node is drawn where it spans at least a camera cell (pxl-engine
/// grid.rs), so the voxels drawn are bounded by the cells: the eye's at the widest view
/// (`EYE_CELLS_MOST`) and the light's claimed tiles', a cell a tile (`LIGHT_CELLS_MOST`). Each
/// cell's count is grown by `CELL_NODES`' parts. The asks keep as many nodes a frame as a walk
/// lists for them (`ray::QUEUE_RECORDS_ROOM`), kept for the two frames an entry is spared
/// (`ASKS_KEPT`).
const _: () = assert!(
    ENTRIES == entries_for(EYE_CELLS_MOST + LIGHT_CELLS_MOST),
    "the slots are what the table must hold"
);

/// The nodes a camera cell may draw, by its parts: a node spanning a cell overlaps up to four
/// cells at their edges (`CELL_OVERLAP`); a place holds a fresh voxel and the stale one it
/// replaces (`raster::STALE`, `CELL_STALE`); and an entry is spared for two frames after it is
/// drawn (`drawn`, `CELL_FRAMES`).
pub const CELL_OVERLAP: u32 = 4;
pub const CELL_STALE: u32 = 2;
pub const CELL_FRAMES: u32 = 2;
pub const CELL_NODES: u32 = CELL_OVERLAP * CELL_STALE * CELL_FRAMES;

/// The nodes the asks keep: a frame's, for the two frames an entry is spared.
pub const ASKS_KEPT: u32 = CELL_FRAMES * crate::ray::QUEUE_RECORDS_ROOM;

/// The eye's camera cells at the widest view (`grid::ASPECT_MOST`), the vertical field's
/// (`grid::HALF_TAN`) across `grid::CELLS_A_RAD`, as `beam::Grid::of` lays them at the cell.
pub const EYE_CELLS_MOST: u32 = ceil(2.0 * grid::HALF_TAN * grid::ASPECT_MOST * grid::CELLS_A_RAD)
    * ceil(2.0 * grid::HALF_TAN * grid::CELLS_A_RAD);

/// The light's cells a cast draws, a cell a tile (`light::BEAM` is `light::TILE`): past them a
/// slot walks.
pub const LIGHT_CELLS_MOST: u32 = 4096;
const _: () = assert!(BEAM == TILE, "a light's cell a tile");

/// The table's load: the share of its slots held, in eighths, with buckets of `BUCKET`.
pub const LOAD_PER_8: u32 = 6;

/// The slots for `cells` camera cells, the asks' nodes with them.
pub const fn entries_for(cells: u32) -> u32 {
    let held = cells * CELL_NODES + ASKS_KEPT;
    (held * 8).div_ceil(LOAD_PER_8).next_power_of_two()
}

const fn ceil(v: f32) -> u32 {
    let n = v as u32;
    if (n as f32) < v {
        n + 1
    } else {
        n
    }
}
/// The boxes an observer's draw writes at most, a drawable entry each (pxl-render
/// record_draw.wgsl; a word each, `raster::draw_word`): the voxels it can draw, `CELL_NODES`
/// a cell of its (`EYE_CELLS_MOST`, `LIGHT_CELLS_MOST`).
pub const EYE_DRAWS: u32 = EYE_CELLS_MOST * CELL_NODES;
pub const LIGHT_DRAWS: u32 = LIGHT_CELLS_MOST * CELL_NODES;

/// The state's bits (`node_cache::STATES_AT`): a face some ray entered a side
/// (`raster::faces`), whether some ray stopped on it coarser than it asked, a cell the query
/// answers whole, and a face an eye's ray entered and asked what lies about it
/// (`node_cache::R_FACES`) a side, as `FACES` numbers them a byte up (`ASKED_SHIFT`). An
/// entry draws where a face is set.
pub const FACES: u32 = (1 << 6) - 1;
pub const WHOLE: u32 = 1 << 7;
pub const ASKED_SHIFT: u32 = 8;
pub const ASKED: u32 = FACES << ASKED_SHIFT;

/// The faces state `state` has an eye's ray asked about, a bit a side as `FACES` lays them:
/// those the eye draws (pxl-render record_draw.wgsl `record_draw_cs`, cast.wgsl).
pub const fn asked_faces(state: u32) -> u32 {
    (state & ASKED) >> ASKED_SHIFT
}

/// Where an ask is a drawable entry's (`beam::ASK_TO`): the bit set on the word face.wgsl
/// leaves the face's word in, among the node cache's words, not the beam rays'; its node then
/// the entry's first word (`beam::ASK_NODE`, `node_cache::entry_at`).
pub const TO_RECORD: u32 = 1 << 31;
const _: () = assert!(
    node_cache::WORDS < TO_RECORD,
    "an ask names any word of the node cache"
);

/// Whether a drawable entry (a face set) last marked in frame `seen` is drawn for frame
/// `frame`, the eye's last draw having run in frame `drawn` (`node_cache::DRAWN`): drawn this
/// frame or the last, or among the boxes the raster draws until the eye's draw runs again. A
/// draw marks each entry it draws, and a stop that makes an entry drawable marks it in its own
/// frame, so none goes before its first draw. A claim takes no drawn entry's slot (pxl-render
/// ask.wgsl `claim_slot`, `drawn`).
pub const fn drawn(seen: u32, frame: u32, drawn: u32) -> bool {
    seen + 2 > frame || seen == drawn
}
const _: () = assert!(
    node_cache::FRAMES + 2 < u32::MAX,
    "a frame two past any a key tells apart is a u32"
);

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

/// Whether a light's ray that asked `band`, the one zoom it asks (`light::Light::band`), and
/// stopped on a node of `zoom` stopped on it whole: coarser than its band.
pub const fn light_whole(zoom: Zoom, band: Zoom) -> bool {
    zoom < band
}

/// Whether a node of `zoom`, `whole` where some ray stopped on it whole (`WHOLE`), is the
/// node a light's ray asking `band` stops on: asked one zoom (`light::Light::band`), the
/// query answers a node at it a cell, so the ray stops at the band or coarser, and coarser
/// only on a node it answers a cell whole (pxl-render record.wgsl `band_fresh`).
pub const fn band_fresh(zoom: Zoom, whole: bool, band: Zoom) -> bool {
    zoom == band || whole && zoom < band
}

/// Whether the light's entry of a node of `zoom`, `whole` as `band_fresh` takes it, is fresh for the
/// light `l`, its map `size` a side, the eye's zooms `range`: the node a ray asking the band
/// of the tile its box's middle `mid` lies on stops on, units from the eye
/// (`light::Light::tile_of`, `light::Light::band`). A light's slot takes only a voxel fresh
/// at its own band (pxl-render light.wgsl), so a stale one is drawn behind every fresh one
/// (`raster::STALE`).
pub fn light_fresh(
    l: &Light,
    size: u32,
    range: [Zoom; 2],
    zoom: Zoom,
    whole: bool,
    mid: [f32; 3],
) -> bool {
    band_fresh(zoom, whole, l.band(size, range, l.tile_of(mid)))
}

/// Whether a ray from the origin `o` that stopped on the node `n` stopped on it whole:
/// coarser than the zoom it asked there, so the query answered it a cell for the cell's own
/// sake (pxl-render world_split.wgsl `world_node`'s `whole`), not for the zoom.
pub fn stopped_whole(o: [i32; 3], n: &Node, range: [Zoom; 2]) -> bool {
    n.zoom < near(o, n, range)
}

/// Whether the entry of the node `n` as a walk from the origin `o` holds it, `whole` where
/// some ray stopped on it whole (`WHOLE`), is the node the walk stops on from `o`, the eye's
/// zooms `range`: it answers a cell there and its parent does not.
pub fn fresh(o: [i32; 3], n: &Node, whole: bool, range: [Zoom; 2]) -> bool {
    let n = *n;
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
                if fresh(o, &n, false, range) {
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
        let range = [0, FINEST_ZOOM];
        assert!(stopped_whole(o, &n, range));
        assert!(!fresh(o, &n, false, range));
        assert!(fresh(o, &n, true, range));
    }

    /// Along a column whose nodes the query answers a cell whole from zoom `whole_from` on,
    /// a light's ray asking a band stops on the node at the band, or at `whole_from` where
    /// that is coarser. An entry some band's ray wrote is fresh at a band only where that
    /// band's ray stops on it, and once that ray's stop has found it (setting `WHOLE` where
    /// it stopped whole), it is fresh there.
    #[test]
    fn band_fresh_is_the_stop_of_every_band() {
        let stop = |band: Zoom, whole_from: Zoom| band.min(whole_from);
        for whole_from in 0..=FINEST_ZOOM {
            for wrote in 0..=FINEST_ZOOM {
                let z = stop(wrote, whole_from);
                let whole = light_whole(z, wrote);
                for band in 0..=FINEST_ZOOM {
                    let stops = stop(band, whole_from) == z;
                    let at = (whole_from, wrote, band);
                    assert!(!band_fresh(z, whole, band) || stops, "{at:?}");
                    let found = whole || stops && light_whole(z, band);
                    assert_eq!(band_fresh(z, found, band), stops, "{at:?} found");
                }
            }
        }
    }

    /// An entry drawn this frame or the last, or by the eye's last draw, is drawn; one drawn
    /// before both is not, nor one no draw drew (frame 0) once the frames pass it.
    #[test]
    fn drawn_spares_the_last_two_frames_and_the_last_draw() {
        assert!(drawn(9, 9, 4));
        assert!(drawn(8, 9, 4));
        assert!(!drawn(7, 9, 4));
        assert!(drawn(4, 9, 4));
        assert!(!drawn(0, 9, 4));
        assert!(drawn(0, 1, 4));
    }
}
