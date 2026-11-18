// pxl-content/generation.rs

//! wgsl-port: the decor the ground holds (`scatter`), generated on the world as either
//! side holds it: where a candidate stands, which way it faces and whether its surface
//! holds it (`placement`, over `DecorQuery`); what placed decor claims (`overlaps_placed`);
//! and how the GPU lays out, lists and draws what it generates in a clip level's bricks.

use pxl_engine::WORKGROUP_CAP;
use pxl_engine::ZOOM_BITS;
use pxl_engine::answer::{self, val};
use pxl_engine::clipmap;
use pxl_engine::clipmap::{ENTRY_CNT, EYE_CLIP_LEVEL_CNT, FIRST_CELL_ZOOM};
use pxl_engine::object;
use pxl_engine::seed::node_seed;
use pxl_engine::world::{PlacedCursor, placed_in};
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::brick;
use pxl_spec::brick::BRICK_SHIFT;
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{FINEST_ZOOM, LEAF_CELL_ZOOM, UNIT_ZOOM, Zoom};
use pxl_spec::value;
use xpute_kit::math::rng::SplitMix32;

use crate::param::scatter::ALIGN_NEIGHBOR_RADIUS;
use crate::scatter::{Candidate, brick_cap, draw_variant, shows_at};
use pxl_spec::asset::decor::{Decor, Orientation, ROWS, Row};

/// The lanes of a decor pass's workgroup.
pub const LANE_CNT: u32 = 64;
const _: () = assert!(
    EYE_ENTRY_CNT.is_multiple_of(LANE_CNT) && brick::BRICK_CELL_CNT.is_multiple_of(LANE_CNT),
    "a pass over the entries or a brick's cells is dispatched whole workgroups"
);

/// The records a run of a decor pass reads (decor_run.wgsl `run_of`); a decor's capacity is a
/// whole number of runs.
pub const RUN: u32 = 16;

/// The most decor one brick generates, all decors together: each decor's capacity
/// (`cap`), laid in the catalog's order.
pub const BRICK_REC_CAP: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < ROWS.len() {
        sum += cap(&ROWS[k]);
        k += 1;
    }
    sum
};

/// The most records any one decor holds in a brick (`cap`), over which `decor_pose_cs` is
/// dispatched.
pub const DECOR_REC_CAP: u32 = {
    let (mut most, mut k) = (0, 0);
    while k < ROWS.len() {
        let cap = cap(&ROWS[k]);
        most = if cap > most { cap } else { most };
        k += 1;
    }
    most
};

const _: () = assert!(
    size_of::<Row>().is_multiple_of(4),
    "a row is whole words, as a GPU reads the table"
);

/// A record of decor the GPU draws: its first word (`rec_word`), then
/// its box's min corner in leaf cells, then how far its base sinks under
/// it (`Prop::sink`).
pub const REC_WORDS: u32 = 5;

/// The eye's clip levels' bricks, one entry each, in which the GPU generates decor; entry `s`
/// is the brick the clip levels' table holds at `s`.
pub const EYE_ENTRY_CNT: u32 = EYE_CLIP_LEVEL_CNT * ENTRY_CNT;
const _: () = assert!(
    EYE_ENTRY_CNT * ROWS.len() as u32 <= WORKGROUP_CAP,
    "every entry's pairs listed, a workgroup deep each, fit one dispatch (decor_mark.wgsl)"
);

/// A record at which no decor stands: its candidate is removed by the thinning or generates
/// nothing where it is, so it is drawn as nothing (its region is `drawing::NONE`).
pub const UNGENERATED: u32 = u32::MAX;
const _: () = assert!(UNGENERATED >> drawing::REGION_SHIFT == drawing::NONE);
const _: () = assert!(
    FINEST_ZOOM < (1 << ZOOM_BITS) - 1,
    "no record's first word is `UNGENERATED`: its zoom's bits are never all set"
);

/// The GPU's decor in one buffer of words: a head for each eye entry, then each entry's
/// `BRICK_REC_CAP` records, then placed decor's head and its records. An entry's records are
/// each decor's in turn, as many as it retained (`entry_rec`); a decor generated again lays
/// its own anew. A head holds the brick the records were generated for, X and Y, and 1 once
/// they were, so that an entry whose brick the clip levels do not hold there any more is
/// skipped (`generated_for`); then how many records each decor laid (`cnt_at`). Placed decor's
/// head is its count.
pub const CNT_AT: u32 = 3;
pub const HDR_WORDS: u32 = CNT_AT + ROWS.len() as u32;
pub const ENTRY_RECS_AT: u32 = EYE_ENTRY_CNT * HDR_WORDS;
/// The records all the entries hold, which come before placed decor's.
pub const ENTRY_REC_CAP: u32 = EYE_ENTRY_CNT * BRICK_REC_CAP;
pub const PLACED_HDR: u32 = ENTRY_RECS_AT + ENTRY_REC_CAP * REC_WORDS;
pub const PLACED_HDR_WORDS: u32 = 4;

/// The runs of records a decor pass's workgroup reads (decor_run.wgsl `run_of`): for an
/// entry, `RUN` records each; for placed decor, one record each.
pub const RUN_CNT: u32 = BRICK_REC_CAP / RUN;

/// Entry `s`'s head.
pub const fn hdr_at(s: u32) -> u32 {
    s * HDR_WORDS
}

/// The word of entry `s`'s head counting the records decor `k` laid.
pub const fn cnt_at(s: u32, k: u32) -> u32 {
    hdr_at(s) + CNT_AT + k
}

/// Where `rows`' decor `k`'s records start among an entry's: the decors'
/// capacities laid in the catalog's order (`cap`).
pub fn first_rec(rows: &[Row], k: u32) -> u32 {
    let (mut first, mut j) = (0, 0);
    while j < k {
        first += cap(&rows[j as usize]);
        j += 1;
    }
    first
}

/// Entry `s`'s `i`-th record of `rows`' decor `k`, as a record number
/// (`rec_at`).
pub fn entry_rec(rows: &[Row], s: u32, k: u32, i: u32) -> u32 {
    s * BRICK_REC_CAP + first_rec(rows, k) + i
}

/// Record `n`'s first word, numbering the entries' records first, entry by entry, then
/// placed decor's.
pub const fn rec_at(n: u32) -> u32 {
    if n < ENTRY_REC_CAP {
        ENTRY_RECS_AT + n * REC_WORDS
    } else {
        PLACED_HDR + PLACED_HDR_WORDS + (n - ENTRY_REC_CAP) * REC_WORDS
    }
}

/// Whether an entry's head says its records were generated for the brick `q`.
pub const fn generated_for(hdr: [u32; 3], q: [i32; 2]) -> bool {
    hdr[2] == 1 && hdr[0] as i32 == q[0] && hdr[1] as i32 == q[1]
}

/// The most boxes a frame's generation is asked over: the damage and cuts held
/// (`object::DAMAGE_CAP`), and each brick handed over this frame, which reaches the candidates
/// whose reads it may answer at every clip level (`reaches`).
pub const RECT_CAP: u32 = object::DAMAGE_CAP + clipmap::FRAME_BRICK_CAP;
/// A box's words: its leaf cells `[x0, y0, x1, y1)`, X on any lap, then the zoom it is filled at,
/// or `EVERY`, the coarsest, for one read at every zoom (`reaches`).
pub const RECT_WORDS: u32 = 5;
pub const EVERY: u32 = 0;
const _: () = assert!(FIRST_CELL_ZOOM > EVERY);

/// What `decor_mark_cs` reads and lists, and what `decor_thin_cs` and `decor_pose_cs` are
/// dispatched by, in words. First, for each of the two, its workgroups across and up and the
/// entry-decor pairs listed, within the list's room (`THIN_ARGS`, `GENERATE_ARGS`); then the
/// pairs it counted as it listed them (`THIN_CNT`, `GENERATE_CNT`). Then come whether every
/// entry generates whole because decor was off (`LIST_EVERY`), how many boxes there are
/// (`LIST_RECTS`), how many pairs generate again in part (`LIST_PARTS`), and a pad; then the
/// boxes, `RECT_WORDS` each. Last is the list, each an entry with its decor above it
/// (`<< 16`), in no order: the pairs generated whole are listed from its start, which
/// `decor_thin_cs` thins and `decor_pose_cs` generates; the pairs generated in part from its end
/// (`part_at`), which `decor_pose_cs` alone generates, its records where a box reaches them
/// (`reaches`).
pub const THIN_ARGS: u32 = 0;
pub const THIN_CNT: u32 = 3;
pub const GENERATE_ARGS: u32 = 4;
pub const GENERATE_CNT: u32 = 7;
pub const LIST_EVERY: u32 = 8;
pub const LIST_RECTS: u32 = 9;
pub const LIST_PARTS: u32 = 10;
pub const RECTS_AT: u32 = 12;
pub const LIST_AT: u32 = RECTS_AT + RECT_CAP * RECT_WORDS;

/// The words of what `decor_mark_cs` lists.
pub const LIST_WORDS: u32 = LIST_AT + EYE_ENTRY_CNT * ROWS.len() as u32;

/// Where the `n`-th pair generated in part is listed: counting from the list's end, since no
/// more pairs are listed whole and in part together than there are pairs.
pub const fn part_at(n: u32) -> u32 {
    LIST_WORDS - 1 - n
}

/// A pair's word in the list: its entry, then its decor past `PAIR_DECOR_SHIFT`.
pub const PAIR_DECOR_SHIFT: u32 = 16;
const _: () = assert!(
    EYE_ENTRY_CNT <= 1 << PAIR_DECOR_SHIFT,
    "an entry fits under a pair's decor"
);

/// Entry `s` and decor `j` as a pair's word.
pub const fn pair_word(s: u32, j: u32) -> u32 {
    s | j << PAIR_DECOR_SHIFT
}

/// A pair's word's entry and decor (`pair_word`).
pub const fn pair_of(w: u32) -> (u32, u32) {
    (w & ((1 << PAIR_DECOR_SHIFT) - 1), w >> PAIR_DECOR_SHIFT)
}

/// How far around its own cell, in its clip level's cells, a candidate's generation reads the
/// world (`placement`): the leaf cells its pose votes on, no farther than as many of its clip
/// level's cells; its box; and the placed decor that may claim it, none wider than the widest
/// decor.
pub const READ_REACH: i32 = ALIGN_NEIGHBOR_RADIUS + WIDEST;

/// The leaf cells, `[x0, y0, x1, y1)`, of brick `q` of the clip level at
/// `cell_zoom`.
pub const fn brick_rect(cell_zoom: Zoom, q: [i32; 2]) -> [i32; 4] {
    let k = BRICK_SHIFT + LEAF_CELL_ZOOM - cell_zoom;
    [q[0] << k, q[1] << k, (q[0] + 1) << k, (q[1] + 1) << k]
}

/// Whether one of the frame's boxes, `b` of leaf cells at `zoom` (`RECT_WORDS`), reaches what a
/// candidate in `at` reads, where `at` is leaf cells of the clip level at `cell_zoom`: a
/// brick's (`brick_rect`), or one candidate's own. A candidate reads the world at its clip
/// level's zoom and finer, each read a cell within `READ_REACH` of its own at that zoom, and a
/// read a finer clip level does not hold falls to a coarser one (`clipmap::step`). So a brick
/// filled at `zoom` may answer a read of a candidate at any clip level: it reaches within
/// `READ_REACH` of the candidate's cells at `zoom` or at `cell_zoom`, whichever is finer. A
/// box of damage or a cut is at `EVERY`, the coarsest, so it reaches within `READ_REACH` of
/// the clip level's own cells.
pub fn reaches(b: [i32; 4], zoom: Zoom, cell_zoom: Zoom, at: [i32; 4]) -> bool {
    let s = LEAF_CELL_ZOOM - zoom.max(cell_zoom);
    let r = READ_REACH << s;
    // `at` widened to the cells it lies in at that zoom.
    let reads = [
        ((at[0] >> s) << s) - r,
        ((at[1] >> s) << s) - r,
        ((((at[2] - 1) >> s) + 1) << s) + r,
        ((((at[3] - 1) >> s) + 1) << s) + r,
    ];
    meets(reads, b)
}

/// Whether the box `b` of leaf cells meets the leaf cells `reads` (`reaches`), each with X
/// on any lap, X wrapping around the world.
pub fn meets(reads: [i32; 4], b: [i32; 4]) -> bool {
    let wide = reads[2] - reads[0];
    // The box's first column counted from the region's, the short way around.
    let (from, width) = (wrap_idx(LEAF_CELL_ZOOM, b[0] - reads[0]), b[2] - b[0]);
    // Met where it starts in the region or over it, or runs on around the world into it.
    let across = width >= 1 << LEAF_CELL_ZOOM
        || (from < wide && from + width > 0)
        || from + width > 1 << LEAF_CELL_ZOOM;
    across && b[1] < reads[3] && b[3] > reads[1]
}

/// A record's first word: its decor and variant, a byte each, its turn in quarters, and the
/// clip level zoom it was generated at, or for placed decor the coarsest it shows at
/// (`scatter::variant_first_cell_zoom`); above them, once sized, its region (`drawing::REGION_SHIFT`).
pub const fn rec_word(decor: u32, variant: u32, turn: u32, cell_zoom: Zoom) -> u32 {
    decor | variant << VARIANT_SHIFT | turn << TURN_SHIFT | cell_zoom << CELL_ZOOM_SHIFT
}

/// The place and width of a record's turn in its first word, and its zoom's place above it
/// (`rec_word`).
pub const TURN_SHIFT: u32 = 16;
/// A record's variant's place past its decor, a byte each (`rec_word`).
pub const VARIANT_SHIFT: u32 = 8;
pub const TURN_BITS: u32 = 2;
pub const CELL_ZOOM_SHIFT: u32 = TURN_SHIFT + TURN_BITS;

/// A record's decor, variant, turn and zoom (`rec_word`).
pub const fn rec_of(w: u32) -> (u32, u32, u32, Zoom) {
    (
        w & ((1 << VARIANT_SHIFT) - 1),
        (w >> VARIANT_SHIFT) & ((1 << VARIANT_SHIFT) - 1),
        (w >> TURN_SHIFT) & ((1 << TURN_BITS) - 1),
        (w >> CELL_ZOOM_SHIFT) & ((1 << ZOOM_BITS) - 1),
    )
}

/// The draw groups: every variant of every decor, numbered decor by decor in the catalog's
/// order (pxl-spec `Decor::first_group`).
pub const GROUP_CNT: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < ROWS.len() {
        sum += ROWS[k].variant_cnt;
        k += 1;
    }
    sum
};

/// How the GPU draws what it generates. Each record is sized for the screen as a model or an
/// impostor, into its group's region: models at `2·group` and impostors just past it. The
/// regions lie end to end in the decor's instances, in the layout the decor and impostor
/// shaders read (lanes x, y, z, spin and scale, `drawing::LANE_CNT` a slot, then the index
/// list), and each region's draw is a set of indirect arguments.
pub mod drawing {
    use super::GROUP_CNT;

    pub const REGION_CNT: u32 = 2 * GROUP_CNT;
    /// A record drawn as nothing.
    pub const NONE: u32 = 0xff;
    /// The place, above its decor, variant, turn and zoom, at which a record's first word holds
    /// its region, once sized.
    pub const REGION_SHIFT: u32 = super::CELL_ZOOM_SHIFT + pxl_engine::ZOOM_BITS;
    pub const LANE_CNT: u32 = 5;
    /// Each region's indirect arguments, `ARGS_WORDS` a region: a model's are indexed, and an
    /// impostor's draw a strip of four corners an instance. The first instance is 0, because a
    /// device without `indirect-first-instance` holds every indirect draw to 0.
    pub const ARGS_WORDS: u32 = 5;
    /// The strip an impostor's instance is drawn by, its corners.
    pub const IMPOSTOR_CORNER_CNT: u32 = 4;
    /// Each region's draw uniform, `UNIFORM_WORDS` apart (`OFF_SZ`, the dynamic offset
    /// alignment every device meets), holding its index list's first slot (pxl-render word.rs
    /// `instance::IDX`): the lanes' base, to which the arguments pass adds the region's first
    /// slot once the regions are laid out.
    pub const UNIFORM_WORDS: u32 = OFF_SZ / 4;
    /// The dynamic offset alignment every WebGPU device meets: its
    /// `minUniformBufferOffsetAlignment` is 256 bytes at most.
    const OFF_SZ: u32 = 256;
    /// The drawing words, by which a frame's decor is drawn: the model's and the impostor's
    /// floors, each an angle around the eye in radians; the anchor the lanes' positions are
    /// counted from (in unit cells, X wrapping); the left-out decor's corner and whether there is
    /// one; then each group's extent.
    pub const MODEL_ANGLE: u32 = 0;
    pub const IMPOSTOR_ANGLE: u32 = 1;
    pub const ANCHOR: u32 = 4;
    pub const HIDE: u32 = 8;
    pub const EXTENTS: u32 = 12;
    pub const WORDS: u32 = EXTENTS + GROUP_CNT;

    /// The least distance, in units, at which a thing's span around the eye is taken
    /// (decor_cnt.wgsl `cnt_one`).
    pub const NEAREST_U: f32 = 1e-3;

    /// What a thing spanning `across` radians around the eye is drawn as: a model at `model` or
    /// more, else an impostor at `impostor` or more, else culled and not drawn, each threshold an
    /// angle. Where drawn, a group's region is `2·group` past its tier.
    pub const MODEL: u32 = 0;
    pub const IMPOSTOR: u32 = 1;
    pub const CULLED: u32 = 2;

    /// The lod of what spans `across` radians, against the model's and the
    /// impostor's floors: the one rule decor's count and the CPU's clouds
    /// both size by.
    pub fn lod(across: f32, model: f32, impostor: f32) -> u32 {
        if across >= model {
            MODEL
        } else if across >= impostor {
            IMPOSTOR
        } else {
            CULLED
        }
    }
}

/// What generating decor asks of the world, as either side holds it: of the base, the
/// ground's alone, its cells' surfaces and the assets around them; of the world as edited,
/// whether an edit holds a cell and what is placed in a box.
pub trait DecorQuery {
    /// A surface as this side keeps it, from which its assets are asked.
    type Surface: Copy;
    /// The surface over cell `c` at `cell_zoom` of the base, with what is
    /// placed left out: the query's support (`support::surface`) as this side asks it, and
    /// its top (`cast::top`); false where nothing fills there or the world lacks the cell.
    fn surface(&mut self, cell_zoom: Zoom, c: [i32; 2]) -> (Self::Surface, (i32, f32), bool);
    /// The finest zoom, `cell_zoom` or finer, at which this side holds the ground's leaf cell
    /// `c`: where a candidate there is asked whether it generates and which way it faces
    /// (`placement`), the same whatever clip level asks.
    fn finest(&mut self, cell_zoom: Zoom, c: [i32; 2]) -> Zoom;
    /// What is in the cell `c` of `zoom`: the base's answer near the surface `f`
    /// (`support::asset`), void or the ground's value; none where the read lacks.
    fn asset(&mut self, f: &Self::Surface, zoom: Zoom, c: [i32; 3]) -> Option<u32>;
    /// What the edited world answers of the cell `c` of `zoom`, down the cell's own line, which
    /// tells an edit there, something placed or air (`answer::is_placed`).
    fn edited(&mut self, zoom: Zoom, c: [i32; 3]) -> u32;
    /// The next leaf placed in `p`'s box, its cell and its word, false past
    /// the last (`world::PlacedCursor::next`).
    fn next_placed(&self, p: &mut PlacedCursor) -> ([i32; 3], u32, bool);
}

/// Decor the ground generates, where it stands: its variant; its heading in quarter turns;
/// its box's min corner in leaf cells, the cell above its surface's; and how far its
/// base lies under that corner, in Q3 steps, where the surface's top is kept short of
/// its cube (`cast::top`), so it stands on the top as drawn, not on its cube's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prop {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
    pub sink: u32,
}

/// Where candidate `c` of `decor` stands at `cell_zoom`. It stands on its cell's surface in
/// the base, the ground's alone, so no edit moves it or takes the ground from under
/// it, and it faces as the cells beside it vote (`asks`). It does not generate where a read of
/// its vote or its box lacks. Then, where `boxed`, it says whether the candidate generates
/// there: every cell of its box is void and every cell under its base is an asset id it
/// generates on, as the ground holds them; no edit lies in its box, air included
/// (`DecorQuery::edited`); and no placed decor's box overlaps its own (`overlaps_placed`). It
/// stands at its clip level's surface, while the vote and its box are asked at the
/// finest zoom at which the world holds its leaf cell (`DecorQuery::finest`), around that
/// zoom's surface, so a candidate faces and generates alike at every clip level it is
/// generated at. Its pose shows as itself only at a clip level its size shows at
/// (`shows_at`). It returns where it stands and whether it generates there, which where not
/// `boxed` is that it shows at `cell_zoom` and no read of its vote lacks; none where its surface is
/// not found.
pub fn placement(
    q: &mut impl DecorQuery,
    decor: &Row,
    cell_zoom: Zoom,
    c: &Candidate,
    boxed: bool,
) -> Option<(Prop, bool)> {
    let cell = |z: Zoom| {
        [
            c.cell[0] >> (LEAF_CELL_ZOOM - z),
            c.cell[1] >> (LEAF_CELL_ZOOM - z),
        ]
    };
    let s = LEAF_CELL_ZOOM - cell_zoom;
    let zoom = q.finest(cell_zoom, c.cell);
    let (f, (floor, rise), here) = q.surface(cell_zoom, cell(cell_zoom));
    if !here {
        return None;
    }
    let at = [c.cell[0], c.cell[1], (floor + 1) * (1 << s)];
    let sink = ((1.0 - rise) * (1u32 << (s + FRAC_W)) as f32 + 0.5) as u32;
    let (f, (base, _), found) = if zoom == cell_zoom {
        (f, (floor, rise), true)
    } else {
        q.surface(zoom, cell(zoom))
    };
    let (variant, turn, generates) = asks(q, &f, decor, (zoom, base), c.seed, at, boxed);
    Some((
        Prop {
            variant,
            turn,
            at,
            sink,
        },
        found && generates && shows_at(decor, variant, cell_zoom),
    ))
}

/// How a candidate standing at `at` is posed by its `seed`, its draws in order: its variant, then its
/// heading in quarter turns. Along transport, the heading is a plurality vote of the cells a
/// way runs on, by their offset from the candidate's along each axis: a quarter turn if
/// horizontal wins, else none (a tie or an isolated cell has no real answer), and a half turn
/// more on a draw, as two-way traffic faces both ways. Then, where `boxed`, whether it
/// generates there (`placement`'s rule). Every cell is asked at `zoom` around the surface
/// `about`, whose top cell is `base`, and a read that lacks keeps it from generating. It
/// returns its variant, its heading, and whether it generates there.
fn asks<Q: DecorQuery>(
    q: &mut Q,
    about: &Q::Surface,
    decor: &Row,
    (zoom, base): (Zoom, i32),
    seed: u32,
    at: [i32; 3],
    boxed: bool,
) -> (u32, u32, bool) {
    let s = LEAF_CELL_ZOOM - zoom;
    let mut draws = SplitMix32::new(seed);
    let variant = draw_variant(decor, &mut draws);
    let [x0, y0, _] = at;
    let foot = [x0 >> s, y0 >> s];
    let mut lacks = false;
    let turn = if decor.orient == Orientation::AlongTransport {
        let flip = draws.next_u32() >> 31;
        let mut way = |x: i32, y: i32| {
            let c = [foot[0] + x, foot[1] + y, base];
            let v = q.asset(about, zoom, c);
            lacks |= v.is_none();
            i32::from(v.is_some_and(|v| value::is_transport(val(v))))
        };
        let (mut horizontal, mut vertical) = (0, 0);
        for d in 1..=ALIGN_NEIGHBOR_RADIUS {
            horizontal += way(-d, 0) + way(d, 0);
            vertical += way(0, -d) + way(0, d);
        }
        u32::from(horizontal > vertical) + 2 * flip
    } else {
        draws.next_u32() >> (u32::BITS - TURN_BITS)
    };
    if lacks || !boxed {
        return (variant, turn, !lacks);
    }
    let [w, d, h] = decor.size(variant, turn).map(|v| v as i32);
    let top = ((base + 1) * (1 << s) + h - 1) >> s;
    for y in foot[1]..=(y0 + d - 1) >> s {
        for x in foot[0]..=(x0 + w - 1) >> s {
            for z in base..=top {
                let c = [x, y, z];
                let Some(v) = q.asset(about, zoom, c) else {
                    return (variant, turn, false);
                };
                // Under its base an asset id it generates on; over it void, and no edit.
                let holds = if z == base {
                    decor.allows(val(v))
                } else {
                    v == Sentinel::Void as u32 && !answer::is_placed(q.edited(zoom, c))
                };
                if !holds {
                    return (variant, turn, false);
                }
            }
        }
    }
    (variant, turn, !overlaps_placed(q, at, [w, d, h]))
}

/// How far decor standing at `at` (leaf cells) lies from `origin` (Q3 steps), in units,
/// squared: from its cell's middle at its base, with X wrapping around the world as the steps
/// do.
pub fn dist2(origin: [i32; 3], at: [i32; 3]) -> f32 {
    let half = 1 << (FRAC_W - 1);
    let step = |v: i32, o: i32, mid: i32| {
        (v.wrapping_shl(FRAC_W).wrapping_add(mid).wrapping_sub(o)) as f32 / (1 << FRAC_W) as f32
    };
    let [dx, dy, dz] = [
        step(at[0], origin[0], half),
        step(at[1], origin[1], half),
        step(at[2], origin[2], 0),
    ];
    dx * dx + dy * dy + dz * dz
}

/// Whether decor is drawn where the rays ask zoom `at` (`Detail::at` at its distance,
/// `dist2`), given its record's zoom `cell_zoom` (`rec_of`). One generated at a coarser
/// clip level is drawn where `at` is that clip level's zoom, so each distance draws one clip
/// level's decor; one generated at the leaves' is drawn where `at` is the leaf cell's zoom or
/// finer; and one `placed` is drawn where `at` is the coarsest zoom it shows at or finer, so
/// it shows where one of its size that the ground generates would. A generated one is asked
/// at the zoom held where it stands (`ray_zoom`), as a ray there draws. Each clip level
/// generates what shows as itself at it (pxl-core domain/view/lens.rs `decor_zooms`,
/// `shows_at`).
pub fn drawn_at(at: Zoom, cell_zoom: Zoom, placed: bool) -> bool {
    if placed {
        at >= cell_zoom
    } else if cell_zoom >= LEAF_CELL_ZOOM {
        at >= LEAF_CELL_ZOOM
    } else {
        at == cell_zoom
    }
}

/// The zoom asked at `at` that the clip levels hold at leaf cell `c`: `at` held to the clip
/// levels (`clipmap::clip_zoom`), or the finest coarser clip level that holds the cell
/// present, as a ray reads it (`clipmap::step`), or the first where none does. Where a finer
/// clip level holds no brick, because the bake left an empty page out (open sea), a coarser
/// clip level's ground is drawn there, and its decor with it.
pub fn ray_zoom(w: &impl clipmap::BrickSource, at: Zoom, c: [i32; 2]) -> Zoom {
    let z = clipmap::clip_zoom(at);
    let s = LEAF_CELL_ZOOM - z;
    let g = [wrap_idx(z, c[0] >> s), c[1] >> s];
    let (f, _) = clipmap::step(w, clipmap::Lookup::NONE, z, g);
    if at < f.from() { at } else { f.from() }
}

/// Placed decor's tag (`node_seed`).
pub const PLACED_TAG: u32 = 0x91ac_ed5e;

/// A placed decor's variant and heading, seeded only by the unit cell of its
/// corner, its one stable identity, so a preview predicts exactly what a
/// placement gets.
pub fn placed_pose(decor: &Row, corner: [i32; 3]) -> (u32, u32) {
    let mut draws = SplitMix32::new(node_seed(PLACED_TAG, (UNIT_ZOOM, corner)));
    let variant = draw_variant(decor, &mut draws);
    (variant, draws.next_u32() >> (u32::BITS - TURN_BITS))
}

/// How far before a box's min corner, in unit cells, a placed decor's corner may lie with
/// its box still meeting the box: any decor's widest side and tallest box, less one.
pub const CLAIM_EXTENT: [i32; 3] = {
    let (mut wide, mut tall, mut k) = (1, 1, 0);
    while k < ROWS.len() {
        let d = &ROWS[k];
        wide = if d.widest() > wide { d.widest() } else { wide };
        let mut v = 0;
        while v < d.variant_cnt as usize {
            tall = if d.sizes[v][2] > tall {
                d.sizes[v][2]
            } else {
                tall
            };
            v += 1;
        }
        k += 1;
    }
    [wide as i32 - 1, wide as i32 - 1, tall as i32 - 1]
};

/// The tallest decor and the widest, in unit cells: as high above what it stands on as
/// anything generated reaches, and as far across (pxl-engine light.rs `Light::box_tiles`;
/// pxl-render decor_ball.wgsl, light_propagate.wgsl `up`).
pub const TALLEST: i32 = CLAIM_EXTENT[2] + 1;
pub const WIDEST: i32 = CLAIM_EXTENT[0] + 1;

/// Whether a placed decor's box overlaps the box `cells` a side from `at` (unit cells): one
/// whose corner, posed as placed (`placed_pose`), holds a box meeting it, each placed leaf
/// near enough being asked of the world (`DecorQuery::next_placed`).
pub(crate) fn overlaps_placed(q: &impl DecorQuery, at: [i32; 3], cells: [i32; 3]) -> bool {
    let lo = [
        at[0] - CLAIM_EXTENT[0],
        at[1] - CLAIM_EXTENT[1],
        at[2] - CLAIM_EXTENT[2],
    ];
    let hi = [
        at[0] + cells[0] - 1,
        at[1] + cells[1] - 1,
        at[2] + cells[2] - 1,
    ];
    let mut placed = placed_in(lo, hi);
    loop {
        let (c, word, more) = q.next_placed(&mut placed);
        if !more {
            return false;
        }
        if let Some(decor) = Decor::find(object::low(word)) {
            let (variant, turn) = placed_pose(decor.row, c);
            let [w, e, v] = decor.row.size(variant, turn);
            let (w, e, v) = (w as i32, e as i32, v as i32);
            if c[0] < at[0] + cells[0]
                && at[0] < c[0] + w
                && c[1] < at[1] + cells[1]
                && at[1] < c[1] + e
                && c[2] < at[2] + cells[2]
                && at[2] < c[2] + v
            {
                return true;
            }
        }
    }
}

/// Its records' room in a brick, a whole number of runs (`RUN`): the most any brick holds
/// (`scatter::brick_cap`).
pub const fn cap(decor: &Row) -> u32 {
    brick_cap(decor).div_ceil(RUN) * RUN
}

#[cfg(test)]
#[path = "generation.test.rs"]
mod test;
