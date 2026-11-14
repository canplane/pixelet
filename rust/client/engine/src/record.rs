// pxl-engine/record.rs

//! wgsl-port: the record of the voxels the observers found, which the raster draws. A voxel
//! is a node cache entry for a node some ray stopped on. The entry holds the World Query's
//! answer for the node and the faces through which rays entered it, and keeps them across
//! frames in the entry's own state word (`node_cache::STATES_AT`). The GPU passes that write
//! and read the record are pxl-render's record*.wgsl, sample_hit.wgsl, cast.wgsl, light.wgsl,
//! face.wgsl, forget.wgsl and ask.wgsl.
//!
//! The rays find and record; only the raster draws. A ray's hit writes the faces where it
//! stops, and every frame the raster draws every drawable entry, not only those the frame's
//! rays reached. A pixel that has to walk the world itself marks a gap in the record, not
//! another way of drawing.
//!
//! **One entry draws a node.** The node cache keys an entry by its node and by the zooms the
//! walk asked at, so one node may have several entries. Their answers agree wherever a ray
//! stops: the query answers a node a ray stops in by its cell, and a cell's answer does not
//! depend on the zoom asked (pxl-render world_split.wgsl `world_node`; world.test.rs
//! `cell_answer_takes_no_zooms`). A node coarser than asked answers only void, and no ray
//! stops in void. Only one of the entries draws. A hit takes its node's entry that answers
//! its cell and already draws, or else the first entry that answers it (pxl-render
//! record.wgsl `find`). No pass that stops writes a key, so all the hits on a node in one
//! pass take the same entry, and a second entry of the node draws only once the first has
//! left the table.
//!
//! An entry holds a fact of the world, not of the view: it stays right as the eye moves and
//! as the light's bands change, until the world changes under it. `WHOLE`, set where a ray
//! stopped on the node coarser than it asked, is such a fact too. The query answers a node
//! coarser than asked as a cell only where nothing finer lies in it (`object::whole`), so
//! any ray that finds the node so may set the bit, the eye's or the light's.
//!
//! **The light** writes and draws the same entries as the eye, with the same code. Its hits
//! are its samples' (pxl-render sample_hit.wgsl over sample.light.wgsl). A light ray asks one
//! zoom, its band (`light::Light::band`), and its hit is whole where the node is coarser
//! than the band (`light_whole`). It sets the faces it enters (`FACES`) but asks none of
//! their words, because the light's slots read no words (sample_hit.light.wgsl). The light's
//! draw (record_draw.wgsl `light_rec_draw_cs`) writes a box for every voxel whose box lies
//! on the light's box, in units from where the ray of box texel (0, 0) starts. A voxel is
//! fresh for the light where a ray asking the band of the tile under the voxel's middle stops
//! on it (`light_fresh`), and stale otherwise; a stale voxel is drawn behind. A light slot
//! takes only a voxel fresh at its own band (pxl-render light.wgsl), so it never takes a
//! stale voxel, and a stale voxel drawn behind never hides a fresh one.
//!
//! Nothing an entry holds depends on the light's direction: the node, its answer and the
//! faces some light ray reached through air are facts of the world, and the direction only
//! places their boxes. So when the sun steps, every entry stays, and the map is redrawn from
//! them along the new direction over the tiles the cast damages (`light::LIST_DAMAGE`). What the
//! new direction meets that no light ray has met yet is not drawn. The epoch that the step
//! begins casts the light's samples over every tile again, nearest first within the budget,
//! so the map converges to the new direction's answer as the eye's picture does.
//!
//! **The writers, in the order a frame runs them:**
//! 1. Forget (pxl-render forget.wgsl `forget_cs`), the node cache's pass. Where the frame
//!    forgets everything, it clears every slot. Otherwise it clears, answer and all, a slot
//!    whose node its rule forgets (`forgets`, `over_rect`, `reads_written`). Where a node
//!    across a face from a drawable entry is forgotten that way, it clears what draws the
//!    entry, its faces, because their words read the cells beside the face; the entry's answer
//!    stays.
//! 2. The walks (pxl-render ask.wgsl `fill`) write an entry's answer and clear what draws it.
//! 3. The hits (pxl-render record_put.wgsl `observe_hit`) are the one way in for every ray:
//!    first the samples' (sample_hit.wgsl), then, after their walk, the hits of the pixels
//!    the samples missed (record_hits.wgsl, `beam::HIT_ASSET`, `HIT_LINE`). A hit carries a
//!    node, the face the ray entered it through, its answer, and the line its face's word is
//!    asked from. It finds its node's entry as above and sets the face it entered. An observer
//!    that reads faces' words, which is the eye, asks a face's word the first time it enters
//!    the face (`ASKED`). That hit settles the word, and it is read only under its bit: none
//!    for a lack or past the asks' room, `face::BOX` for a cell short of its cube, and
//!    otherwise what face.wgsl writes into `node_cache::R_FACES`. A hit on a node no slot
//!    keeps sets nothing, and its pixel walks again.
//! 4. The draws (record_draw.wgsl `rec_draw_cs`, `light_rec_draw_cs`) read every slot,
//!    write a box for each drawable entry and mark the entry drawn (`node_cache::R_DRAWN`).
//!    The eye's draw also records the frame it ran in (`node_cache::DRAWN`). The raster draws
//!    the boxes, and `cast_cs` and light.wgsl read a pixel's slot.
//!
//! Each word has one writer in each step.
//!
//! **Fresh or stale:** an entry is fresh where its node is the one the walk from this eye
//! stops on (`fresh`). That holds where the node answers a cell at its zoom and its parent
//! does not. A node answers a cell where its zoom is at or past the zoom asked at its nearest
//! point, or where a ray stopped on it whole. Every ancestor of a node is nearer to the eye at
//! its own nearest point, so it is asked no coarser a zoom, and the parent's test holds for it
//! as well. So two fresh entries never nest.
//!
//! A stale entry is never a pixel's answer. A pixel no fresh voxel covers is a hole, which its
//! walk answers in its frame (pxl-render cast.wgsl `cast_px`), so the eye draws no stale entry
//! and leaves it to be evicted (`drawn`). The light draws its stale entries behind every fresh
//! one (`raster::STALE`): a tile is cast once in an epoch, and a stale box keeps a texel no
//! fresh voxel covers from being taken for the sky (pxl-render light.wgsl).
//!
//! **Damage:** every write to what is placed that changes the tree or a record names the cube
//! it wrote (pxl-core domain/layer/object/store.rs `Store::write`, `set_fold`, `clear_fold`).
//! The GPU is handed the cube as the columns of its leaf cells; past the room for them,
//! columns are joined, never dropped (pxl-core domain/scene/decor/gpu.rs `note_damage`;
//! pxl-render gpu/generator.rs `hold`). A write that names no cube, or that rewrites more child
//! groups than there is room for, forgets everything. A cell's answer reads the tree only along
//! the cell's own path, and a write changes that path only within the cube and above it;
//! `over_rect` on the cell's column catches both. `reads_written` forgets an entry that was
//! handed a slot of a rewritten child group. Drawable entries are forgotten by the same rule
//! as the rest.
//!
//! **The faces drawn** are the sides of a node (one of six, `ray::face`) through which some
//! ray entered it. A ray entering through a side proves that the side borders air at the
//! node's zoom. That is a fact of the world until an edit forgets the entry, so the face is
//! drawn from any eye it is turned to, and the depth test decides the rest. The light draws
//! every face any ray entered (`FACES`). The eye shows what lies around a face, which it asks
//! as one word a side, so it draws only the faces an eye's ray entered and asked about
//! (`ASKED`). A face only the light entered is a hole to the eye until an eye's ray reaches it
//! and asks. A side no ray entered is not drawn, even the side opposite an entered one, which
//! may lie against something solid. An eye that moves past the node's plane, or a sun that
//! turns across it, sees that side only once a ray reaches it; until then it is a hole, and
//! closing it is the rays' work.
//!
//! **A drawn voxel hides no nearer one from the rays.** The samples walk from their cells'
//! starts, which the beam rays find from the eye, not from what is drawn, so a sample stops on
//! the first cell along its ray whatever is drawn there. An occluder that a sample's ray
//! meets is drawn in the frame it is cast, and a pixel the samples miss is walked and stopped
//! in the same frame.
//!
//! **Eviction** is the node cache's (`node_cache`), but it never takes an entry drawn in the
//! last two frames (`drawn`). A draw marks each entry it draws with its frame, and a hit that
//! makes an entry drawable marks it with its own frame. While no draw runs, the raster keeps
//! drawing the boxes of the eye's last draw, whose frame the node cache's head holds
//! (`node_cache::DRAWN`), and those entries are kept too. Where a bucket holds no entry it may
//! take, the walk keeps nothing, and its pixels walk again.

use crate::cast::{self, Detail, Node, Spacing};
use crate::light::{Light, BEAM, TILE};
use crate::node_cache::{self, ENTRY_CNT};
use crate::ray::QUEUE_RECS_CAP;
use pxl_spec::space::camera::grid;

use crate::WORKGROUP_CAP;
use pxl_spec::space::coord::zoom::Zoom;

/// The threads in a workgroup of each pass over every slot (pxl-render record_draw.wgsl).
pub const DRAW_LANE_CNT: u32 = 64;
const _: () = assert!(
    ENTRY_CNT.is_multiple_of(DRAW_LANE_CNT) && ENTRY_CNT / DRAW_LANE_CNT <= WORKGROUP_CAP,
    "the passes over every slot take each once in one dimension of one dispatch"
);

/// The node cache's slots (`node_cache::ENTRY_CNT`): what the table must hold, divided by its
/// load (`LOAD_PER_8` eighths of the slots in use), rounded up to a power of two
/// (`entries_for`).
///
/// The table holds every voxel a view can draw, the eye's and the light's, and the nodes a
/// frame's asks keep. A node is drawn only where it spans at least one camera cell (grid.rs),
/// so the cells bound the voxels drawn: the eye's cells at the widest view (`EYE_CELLS_CAP`),
/// and the light's damaged tiles at one cell a tile (`LIGHT_CELLS_CAP`). Each cell's count is
/// multiplied by the factors of `CELL_NODE_CAP`. The asks keep as many nodes a frame as a walk
/// lists for them (`ray::QUEUE_RECS_CAP`), for the two frames an entry is spared
/// (`ASKS_KEPT`).
const _: () = assert!(
    ENTRY_CNT == entries_for(EYE_CELLS_CAP + LIGHT_CELLS_CAP),
    "the slots are what the table must hold"
);

/// The nodes one camera cell may draw, as factors. A node spanning a cell overlaps up to four
/// cells at their edges (`CELL_OVERLAP`). A place holds a fresh voxel and the stale one it
/// replaces (`raster::STALE`, `CELL_STALE`). An entry is spared for two frames after it is
/// drawn (`drawn`, `CELL_FRAME_CNT`).
pub const CELL_OVERLAP: u32 = 4;
pub const CELL_STALE: u32 = 2;
pub const CELL_FRAME_CNT: u32 = 2;
pub const CELL_NODE_CAP: u32 = CELL_OVERLAP * CELL_STALE * CELL_FRAME_CNT;

/// The nodes the asks keep: one frame's worth for each of the two frames an entry is spared.
pub const ASKS_KEPT: u32 = CELL_FRAME_CNT * QUEUE_RECS_CAP;

/// The eye's camera cells at the widest view (`grid::ASPECT_MOST`) across the vertical field
/// of view (`grid::HALF_TAN`), at `grid::CELLS_A_RAD` cells a radian, as `beam::Grid::of` lays
/// them out.
pub const EYE_CELLS_CAP: u32 = ceil(2.0 * grid::HALF_TAN * grid::ASPECT_MOST * grid::CELLS_A_RAD)
    * ceil(2.0 * grid::HALF_TAN * grid::CELLS_A_RAD);

/// The most light cells a cast draws, one cell a tile (`light::BEAM` equals `light::TILE`). A
/// light slot past them walks on its own.
pub const LIGHT_CELLS_CAP: u32 = 4096;
const _: () = assert!(BEAM == TILE, "a light's cell a tile");

/// The table's load: the share of its slots in use, in eighths, with buckets of `BUCKET`.
pub const LOAD_PER_8: u32 = 6;

/// The slots needed for `cells` camera cells and for the nodes the asks keep.
pub const fn entries_for(cells: u32) -> u32 {
    let nodes = cells * CELL_NODE_CAP + ASKS_KEPT;
    (nodes * 8).div_ceil(LOAD_PER_8).next_power_of_two()
}

const fn ceil(v: f32) -> u32 {
    let n = v as u32;
    if (n as f32) < v {
        n + 1
    } else {
        n
    }
}
/// The most boxes an observer's draw writes, one word for each drawable entry
/// (`raster::draw_word`): `CELL_NODE_CAP` for each of its cells (`EYE_CELLS_CAP`,
/// `LIGHT_CELLS_CAP`), but for the stale ones, which the eye does not draw.
pub const EYE_DRAW_CAP: u32 = EYE_CELLS_CAP * CELL_NODE_CAP / CELL_STALE;
pub const LIGHT_DRAW_CAP: u32 = LIGHT_CELLS_CAP * CELL_NODE_CAP;

/// The bits of an entry's state word (`node_cache::STATES_AT`). `FACES` holds a bit for each
/// side some ray entered (`raster::faces`). `WHOLE` is set where some ray stopped on the node
/// coarser than it asked, a node the query answers as a cell whole. `ASKED` holds, a byte up
/// (`ASKED_SHIFT`), a bit for each side an eye's ray entered and asked what lies around it
/// (`node_cache::R_FACES`), in the order of `FACES`. An entry draws where any face bit is set.
pub const FACES: u32 = (1 << 6) - 1;
pub const WHOLE: u32 = 1 << 7;
pub const ASKED_SHIFT: u32 = 8;
pub const ASKED: u32 = FACES << ASKED_SHIFT;

/// The faces of `state` that an eye's ray asked about, one bit a side in the order of
/// `FACES`. These are the faces the eye draws (pxl-render record_draw.wgsl, cast.wgsl).
pub const fn asked_faces(state: u32) -> u32 {
    (state & ASKED) >> ASKED_SHIFT
}

/// The bit of an ask's target (`beam::ASK_TO`) that marks the ask as a drawable entry's. With
/// it set, the target is a word of the node cache, where face.wgsl leaves the face's word,
/// not of the beam rays' buffer, and the ask names the entry's first word, past which the entry
/// holds its node at `node_cache::W_NODE` (`beam::ASK_NODE`, `node_cache::entry_at`).
pub const TO_REC: u32 = 1 << 31;
const _: () = assert!(
    node_cache::WORDS < TO_REC,
    "an ask names any word of the node cache"
);

/// Whether a drawable entry last marked in frame `seen` still counts as drawn in frame
/// `frame`, where the eye's last draw ran in frame `drawn` (`node_cache::DRAWN`). It does if it
/// was marked this frame or the last, or if it is among the boxes the raster keeps drawing
/// until the eye's draw runs again. A hit that makes an entry drawable marks it in its own
/// frame, so no entry is evicted before its first draw. A claim never takes the slot of a
/// drawn entry (pxl-render ask.wgsl `claim_slot`).
pub const fn drawn(seen: u32, frame: u32, drawn: u32) -> bool {
    seen + 2 > frame || seen == drawn
}
const _: () = assert!(
    node_cache::FRAME_CNT + 2 < u32::MAX,
    "a frame two past any a key tells apart is a u32"
);

/// The zoom that a walk from the origin `o` asks at the point of the node `n` nearest to `o`,
/// within the eye's zooms `range`.
fn near(o: [i32; 3], n: &Node, range: [Zoom; 2]) -> Zoom {
    cast::node_range(
        o,
        n,
        Detail {
            range,
            spacing: Spacing::NONE,
        },
    )[1]
}

/// Whether a light ray that asked `band`, the one zoom a light ray asks
/// (`light::Light::band`), stopped whole on a node of `zoom`: on a node coarser than its band.
pub const fn light_whole(zoom: Zoom, band: Zoom) -> bool {
    zoom < band
}

/// Whether a light ray asking `band` stops on a node of `zoom`, where `whole` says some ray
/// stopped on the node whole (`WHOLE`). A light ray asks one zoom, and the query answers a
/// node at that zoom as a cell, so the ray stops at its band or coarser, and coarser only on a
/// node the query answers whole. pxl-render record.wgsl `band_fresh` is its twin.
pub const fn band_fresh(zoom: Zoom, whole: bool, band: Zoom) -> bool {
    zoom == band || whole && zoom < band
}

/// Whether the light's entry for a node of `zoom` is fresh for the light `l`, whose map is
/// `size` a side, within the eye's zooms `range`; `whole` is as for `band_fresh`. The entry is
/// fresh where a ray asking the band of the tile under the box's middle `mid`, in units from
/// the eye, stops on its node. A light slot takes only a voxel fresh at its own band
/// (pxl-render light.wgsl), so a stale one is drawn behind every fresh one (`raster::STALE`).
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

/// Whether a ray from the origin `o` that stopped on the node `n` stopped on it whole, that
/// is, on a node coarser than the zoom it asked there. The query then answered the node as a
/// cell because nothing finer lies in it, not because of the zoom asked.
pub fn ray_whole(o: [i32; 3], n: &Node, range: [Zoom; 2]) -> bool {
    n.zoom < near(o, n, range)
}

/// Whether the entry of the node `n` is fresh: whether `n` is the node a walk from the origin
/// `o` stops on, within the eye's zooms `range`. It is where `n` answers a cell, at its zoom
/// or because `whole` says some ray stopped on it whole, and its parent does not.
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

/// How much the draw widens the screen's tangents, as a share, when it asks whether a box is
/// in view (`in_view`), to cover the raster's jitter and growth (`raster::GROWTH`).
pub const VIEW_GROWTH: f32 = 1.0 / 64.0;

/// Whether the box `lo`..`hi`, in units from the eye, meets what the eye sees as far as
/// `far`. The eye looks along the axes `right`, `up` and `forward`, through a screen whose
/// tangents `tan` are widened by `VIEW_GROWTH`. The box is in view where some point of it lies
/// before the eye, its nearest point along `forward` is short of `far`, and some point of it
/// lies inside each of the screen's four sides. pxl-render record_draw.wgsl `in_view` is its
/// twin.
pub fn in_view(
    (lo, hi): ([f32; 3], [f32; 3]),
    [right, up, forward]: [[f32; 3]; 3],
    tan: [f32; 2],
    far: f32,
) -> bool {
    // The largest `n · p` over the box's points.
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
    use pxl_spec::space::coord::zoom::{FINEST_ZOOM, UNIT_ZOOM};
    use xpute_kit::math::rng::mix64;

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

    /// Two fresh entries never nest. From any origin, at most one of a node and the nodes above
    /// it is fresh, and the node a walk would stop on along the column is the fresh one.
    #[test]
    fn fresh_never_nests() {
        let range = [4, FINEST_ZOOM];
        for i in 0..2048u64 {
            let h = mix64(i);
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
            // The walk stops at the first zoom at or past the zoom asked at the node's nearest point.
            let hit = (0..=FINEST_ZOOM).find(|&z| {
                let n = node(z, leaf.corner, leaf.roots);
                !ray_whole(o, &n, range)
            });
            assert_eq!(first, hit, "{o:?}");
        }
    }

    /// A node a ray stopped on whole is fresh whatever zoom is asked at it, as long as its parent
    /// is not.
    #[test]
    fn whole_is_fresh_where_its_parent_splits() {
        let o = [0, 0, 1 << 12];
        let n = node(
            UNIT_ZOOM - 2,
            [0, cast::Y_START, cast::Z_START + (1 << 12)],
            [0; 3],
        );
        let range = [0, FINEST_ZOOM];
        assert!(ray_whole(o, &n, range));
        assert!(!fresh(o, &n, false, range));
        assert!(fresh(o, &n, true, range));
    }

    /// Along a column whose nodes the query answers whole from zoom `whole_from` on, a light ray
    /// asking a band stops on the node at the band, or at `whole_from` where that is coarser. An
    /// entry written by one band's ray is fresh at a band only where that band's ray stops on it.
    /// Once that ray's hit has found the entry, setting `WHOLE` where it stopped whole, the entry
    /// is fresh there.
    #[test]
    fn band_fresh_is_the_hit_of_every_band() {
        let hit_zoom = |band: Zoom, whole_from: Zoom| band.min(whole_from);
        for whole_from in 0..=FINEST_ZOOM {
            for wrote in 0..=FINEST_ZOOM {
                let z = hit_zoom(wrote, whole_from);
                let whole = light_whole(z, wrote);
                for band in 0..=FINEST_ZOOM {
                    let hit = hit_zoom(band, whole_from) == z;
                    let at = (whole_from, wrote, band);
                    assert!(!band_fresh(z, whole, band) || hit, "{at:?}");
                    let found = whole || hit && light_whole(z, band);
                    assert_eq!(band_fresh(z, found, band), hit, "{at:?} found");
                }
            }
        }
    }

    /// An entry marked this frame or the last, or drawn by the eye's last draw, counts as drawn.
    /// One marked earlier does not, nor one no draw ever marked (frame 0) once two frames pass.
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
