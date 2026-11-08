// pxl-content/generation.rs

//! gpu-portable: the decor the ground holds (`scatter`), generated on the world as either
//! side holds it: where a candidate stands, which way it faces and whether its column surface
//! holds it (`placement`, over `DecorQuery`); what placed decor claims (`overlaps_placed`);
//! and how the GPU lays out, lists and draws what it generates in a clip level's bricks.

use pxl_engine::answer::{self, val};
use pxl_engine::clipmap;
use pxl_engine::clipmap::{BRICK, BRICK_SHIFT, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL_ZOOM};
use pxl_engine::object;
use pxl_engine::seed::node_seed;
use pxl_engine::value;
use pxl_engine::world::{placed_in, PlacedCursor};
use pxl_engine::WORKGROUPS_CAP;
use pxl_engine::{wrap_idx, Zoom, FINEST_ZOOM, FRAC_W, LEAF_CELL_ZOOM, UNIT_ZOOM, ZOOM_BITS};
use xpute_kit::math::rng::SplitMix32;

use crate::decor::{Decor, Orientation, DECORS};
use crate::param::scatter::ALIGN_NEIGHBOR_RADIUS;
use crate::scatter::{mul_hi, root, stands_at, variant_of, Candidate};

/// The lanes of a decor pass's workgroup.
pub const LANES: u32 = 64;
const _: () = assert!(
    EYE_ENTRIES.is_multiple_of(LANES) && clipmap::BRICK_CELLS.is_multiple_of(LANES),
    "a pass over the entries or a brick's cells is dispatched whole workgroups"
);

/// The records a run of a decor pass reads (decor_run.wgsl `run_of`); a decor's capacity is a
/// whole number of runs.
pub const RUN: u32 = 16;

/// The most decor one brick generates, all decors together: each decor's capacity
/// (`Decor::cap`), laid in the catalog's order.
pub const BRICK_RECORDS: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        sum += DECORS[k].cap();
        k += 1;
    }
    sum
};

/// The most records any one decor holds in a brick (`Decor::cap`), over which `decor_cs` is
/// dispatched.
pub const KIND_RECORDS: u32 = {
    let (mut most, mut k) = (0, 0);
    while k < DECORS.len() {
        let cap = DECORS[k].cap();
        most = if cap > most { cap } else { most };
        k += 1;
    }
    most
};

const _: () = assert!(
    size_of::<Decor>().is_multiple_of(4),
    "a row is whole words, as a GPU reads the table"
);

/// A record of decor the GPU draws: its first word (`record_word`), then
/// its box's min corner in leaf cells, then how far its base sinks under
/// it (`Prop::sink`).
pub const RECORD_WORDS: u32 = 5;

/// The eye's clip levels' bricks, one entry each, in which the GPU generates decor; entry `s`
/// is the brick the clip levels' table holds at `s`.
pub const EYE_ENTRIES: u32 = EYE_CLIP_LEVELS * ENTRIES;
const _: () = assert!(
    EYE_ENTRIES * DECORS.len() as u32 <= WORKGROUPS_CAP,
    "every entry's pairs listed, a workgroup deep each, fit one dispatch (decor_mark.wgsl)"
);

/// A record at which no decor stands: its candidate is removed by the thinning or generates
/// nothing where it is, so it is drawn as nothing (its region is `drawn::NONE`).
pub const UNGENERATED: u32 = u32::MAX;
const _: () = assert!(UNGENERATED >> drawn::REGION_SHIFT == drawn::NONE);
const _: () = assert!(
    FINEST_ZOOM < (1 << ZOOM_BITS) - 1,
    "no record's first word is `UNGENERATED`: its zoom's bits are never all set"
);

/// The GPU's decor in one buffer of words: a head for each eye entry, then each entry's
/// `BRICK_RECORDS` records, then placed decor's head and its records. An entry's records are
/// each decor's in turn, as many as it retained (`entry_record`); a decor generated again lays
/// its own anew. A head holds the brick the records were generated for, X and Y, and 1 once
/// they were, so that an entry whose brick the clip levels do not hold there any more is
/// skipped (`generated_for`); then how many records each decor laid (`cnt_at`). Placed decor's
/// head is its count.
pub const CNT_AT: u32 = 3;
pub const HEADER_WORDS: u32 = CNT_AT + DECORS.len() as u32;
pub const SLOTS_AT: u32 = EYE_ENTRIES * HEADER_WORDS;
/// The records all the entries hold, which come before placed decor's.
pub const SLOT_RECORDS: u32 = EYE_ENTRIES * BRICK_RECORDS;
pub const PLACED_HEADER: u32 = SLOTS_AT + SLOT_RECORDS * RECORD_WORDS;
pub const PLACED_HEADER_WORDS: u32 = 4;

/// The runs of records a decor pass's workgroup reads (decor_run.wgsl `run_of`): for an
/// entry, `RUN` records each; for placed decor, one record each.
pub const RUNS: u32 = BRICK_RECORDS / RUN;

/// Entry `s`'s head.
pub const fn header_at(s: u32) -> u32 {
    s * HEADER_WORDS
}

/// The word of entry `s`'s head counting the records decor `k` laid.
pub const fn cnt_at(s: u32, k: u32) -> u32 {
    header_at(s) + CNT_AT + k
}

/// Where `rows`' decor `k`'s records start among an entry's: the decors'
/// capacities laid in the catalog's order (`Decor::cap`).
pub fn first_record(rows: &[Decor], k: u32) -> u32 {
    let (mut first, mut j) = (0, 0);
    while j < k {
        first += rows[j as usize].cap();
        j += 1;
    }
    first
}

/// Entry `s`'s `i`-th record of `rows`' decor `k`, as a record number
/// (`record_at`).
pub fn entry_record(rows: &[Decor], s: u32, k: u32, i: u32) -> u32 {
    s * BRICK_RECORDS + first_record(rows, k) + i
}

/// Record `n`'s first word, numbering the entries' records first, entry by entry, then
/// placed decor's.
pub const fn record_at(n: u32) -> u32 {
    if n < SLOT_RECORDS {
        SLOTS_AT + n * RECORD_WORDS
    } else {
        PLACED_HEADER + PLACED_HEADER_WORDS + (n - SLOT_RECORDS) * RECORD_WORDS
    }
}

/// Whether an entry's head says its records were generated for the brick `q`.
pub const fn generated_for(header: [u32; 3], q: [i32; 2]) -> bool {
    header[2] == 1 && header[0] as i32 == q[0] && header[1] as i32 == q[1]
}

/// The most boxes a frame's generation is asked over: the edits and cuts held
/// (`object::RECTS`), and each brick handed over this frame, which reaches its clip level's
/// zoom and every coarser one (`asks`'s vote).
pub const RECT_CAP: u32 = object::RECTS + clipmap::FRAME_BRICKS;
/// A box's words: its leaf cells `[x0, y0, x1, y1)`, X on any lap, then the finest clip level
/// zoom whose candidates it reaches, or `EVERY` for all.
pub const RECT_WORDS: u32 = 5;
pub const EVERY: u32 = 0;
const _: () = assert!(FIRST_CELL_ZOOM > EVERY);

/// What `decor_mark_cs` reads and lists, and what `decor_thin_cs` and `decor_cs` are
/// dispatched by, in words. First, for each of the two, its workgroups across and up and the
/// entry-decor pairs listed, within the list's room (`THIN_ARGS`, `GENERATE_ARGS`); then the
/// pairs it counted as it listed them (`THIN_CNT`, `GENERATE_CNT`). Then come whether every
/// entry generates whole because decor was off (`LIST_EVERY`), how many boxes there are
/// (`LIST_RECTS`), how many pairs generate again in part (`LIST_PARTS`), and a pad; then the
/// boxes, `RECT_WORDS` each. Last is the list, each an entry with its decor above it
/// (`<< 16`), in no order: the pairs generated whole are listed from its start, which
/// `decor_thin_cs` thins and `decor_cs` generates; the pairs generated in part from its end
/// (`part_at`), which `decor_cs` alone generates, its records where a box reaches them
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
pub const LIST_WORDS: u32 = LIST_AT + EYE_ENTRIES * DECORS.len() as u32;

/// Where the `n`-th pair generated in part is listed: counting from the list's end, since no
/// more pairs are listed whole and in part together than there are pairs.
pub const fn part_at(n: u32) -> u32 {
    LIST_WORDS - 1 - n
}

/// A pair's word in the list: its entry, then its decor past `PAIR_DECOR_SHIFT`.
pub const PAIR_DECOR_SHIFT: u32 = 16;
const _: () = assert!(
    EYE_ENTRIES <= 1 << PAIR_DECOR_SHIFT,
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

/// Whether one of the frame's boxes, `b` of leaf cells at `zoom` (`RECT_WORDS`), reaches what
/// a candidate in `at` reads, where `at` is leaf cells of the clip level at `cell_zoom`: a
/// brick's (`brick_rect`), or one candidate's own. A candidate reads the world at its clip
/// level's zoom and finer, each read a cell within `READ_REACH` of its own at that zoom. A
/// brick filled at `zoom` answers only reads at `zoom` or finer (`clipmap::step`), so it
/// reaches the clip levels at `zoom` and coarser, within `READ_REACH` of its own cells, not of
/// theirs. An edit or a cut (`EVERY`) is read at every zoom, so it reaches within
/// `READ_REACH` of the clip level's own cells.
pub fn reaches(b: [i32; 4], zoom: Zoom, cell_zoom: Zoom, at: [i32; 4]) -> bool {
    if zoom != EVERY && zoom < cell_zoom {
        return false;
    }
    let s = LEAF_CELL_ZOOM - if zoom == EVERY { cell_zoom } else { zoom };
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
/// clip level zoom it was generated at, or for placed decor the coarsest it stands at
/// (`scatter::coarsest_stand`); above them, once sized, its region (`drawn::REGION_SHIFT`).
pub const fn record_word(decor: u32, variant: u32, turn: u32, cell_zoom: Zoom) -> u32 {
    decor | variant << VARIANT_SHIFT | turn << TURN_SHIFT | cell_zoom << CELL_ZOOM_SHIFT
}

/// The place and width of a record's turn in its first word, and its zoom's place above it
/// (`record_word`).
pub const TURN_SHIFT: u32 = 16;
/// A record's variant's place past its decor, a byte each (`record_word`).
pub const VARIANT_SHIFT: u32 = 8;
pub const TURN_BITS: u32 = 2;
pub const CELL_ZOOM_SHIFT: u32 = TURN_SHIFT + TURN_BITS;

/// A record's decor, variant, turn and zoom (`record_word`).
pub const fn record_of(w: u32) -> (u32, u32, u32, Zoom) {
    (
        w & ((1 << VARIANT_SHIFT) - 1),
        (w >> VARIANT_SHIFT) & ((1 << VARIANT_SHIFT) - 1),
        (w >> TURN_SHIFT) & ((1 << TURN_BITS) - 1),
        (w >> CELL_ZOOM_SHIFT) & ((1 << ZOOM_BITS) - 1),
    )
}

/// The draw groups: every variant of every decor, numbered decor by decor in the catalog's
/// order (`first_group`).
pub const GROUPS: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        sum += DECORS[k].variants;
        k += 1;
    }
    sum
};

/// The draw group of `rows`' decor `decor`'s first variant.
pub fn first_group(rows: &[Decor], decor: u32) -> u32 {
    let (mut first, mut k) = (0, 0);
    while k < decor {
        first += rows[k as usize].variants;
        k += 1;
    }
    first
}

/// How the GPU draws what it generates. Each record is sized for the screen as a model or an
/// impostor, into its group's region: models at `2·group` and impostors just past it. The
/// regions lie end to end in the decor's instances, in the layout the decor and impostor
/// shaders read (columns x, y, z, spin and scale, `drawn::COLUMNS` a slot, then the index
/// list), and each region's draw is a set of indirect arguments.
pub mod drawn {
    use super::GROUPS;

    pub const REGIONS: u32 = 2 * GROUPS;
    /// A record drawn as nothing.
    pub const NONE: u32 = 0xff;
    /// The place, above its decor, variant, turn and zoom, at which a record's first word holds
    /// its region, once sized.
    pub const REGION_SHIFT: u32 = super::CELL_ZOOM_SHIFT + pxl_engine::ZOOM_BITS;
    pub const COLUMNS: u32 = 5;
    /// Each region's indirect arguments, `ARGS_WORDS` a region: a model's are indexed, and an
    /// impostor's draw a strip of four corners an instance. The first instance is 0, because a
    /// device without `indirect-first-instance` holds every indirect draw to 0.
    pub const ARGS_WORDS: u32 = 5;
    /// The strip an impostor's instance is drawn by, its corners.
    pub const IMPOSTOR_CORNERS: u32 = 4;
    /// Each region's draw uniform, `UNIFORM_WORDS` apart (`OFF_BYTES`, the dynamic offset
    /// alignment every device meets), holding its index list's first slot (pxl-render word.rs
    /// `instance::IDX`): the columns' base, to which the arguments pass adds the region's first
    /// slot once the regions are laid out.
    pub const UNIFORM_WORDS: u32 = OFF_BYTES / 4;
    /// The dynamic offset alignment every WebGPU device meets: its
    /// `minUniformBufferOffsetAlignment` is 256 bytes at most.
    const OFF_BYTES: u32 = 256;
    /// The drawing words, by which a frame's decor is drawn: the model's and the impostor's
    /// floors, each an angle around the eye in radians; the anchor the columns' positions are
    /// counted from (in unit cells, X wrapping); the left-out decor's corner and whether there is
    /// one; then each group's extent.
    pub const MODEL_ANGLE: u32 = 0;
    pub const IMPOSTOR_ANGLE: u32 = 1;
    pub const ANCHOR: u32 = 4;
    pub const HIDE: u32 = 8;
    pub const EXTENTS: u32 = 12;
    pub const DRAWING_WORDS: u32 = EXTENTS + GROUPS;

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

/// What generating decor asks of the world, as either side holds it: of the world unplaced
/// (`world::Inheritance::UNPLACED`), the ground's alone, its columns' surfaces and the assets
/// around them; of the world as edited, whether an edit holds a cell and what is placed in a
/// box; and the catalog's rows, where it reads them.
pub trait DecorQuery {
    /// A column surface as this side keeps it, from which its assets are asked.
    type ColumnSurface: Copy;
    /// The catalog's rows (`decor::DECORS`), as this side holds them: a GPU
    /// reads them from a buffer.
    fn rows(&self) -> &[Decor];
    /// The column surface down column `g` at `cell_zoom` of the world unplaced, with what is
    /// placed left out: the query's support (`support::column_surface`) as this side asks it, and
    /// its top (`cast::top`); false where nothing fills there or the world lacks the column.
    fn column_surface(
        &mut self,
        cell_zoom: Zoom,
        g: [i32; 2],
    ) -> (Self::ColumnSurface, (i32, f32), bool);
    /// The finest zoom, `cell_zoom` or finer, at which this side holds the ground's leaf column
    /// `g`: where a candidate there is asked whether it stands and which way it faces
    /// (`placement`), the same whatever clip level asks.
    fn finest(&mut self, cell_zoom: Zoom, g: [i32; 2]) -> Zoom;
    /// What is in the cell `c` of `zoom`: the unplaced world's answer near the column surface `f`
    /// (`support::asset`), void or the ground's value; or, where `edited`, the edited world's
    /// answer down the cell's own line, which tells an edit there, something placed or air
    /// (`answer::is_placed`). One ask serves both, so a GPU's walk to the cell is written once.
    fn asset(&mut self, f: &Self::ColumnSurface, zoom: Zoom, c: [i32; 3], edited: bool) -> u32;
    /// The next leaf placed in `p`'s box, its cell and its word, false past
    /// the last (`world::PlacedCursor::next`).
    fn next_placed(&self, p: &mut PlacedCursor) -> ([i32; 3], u32, bool);
}

/// Decor the ground generates, where it stands: its variant; its heading in quarter turns;
/// its box's min corner in leaf cells, the cell above its column surface's; and how far its
/// base lies under that corner, in Q3 steps, where the column surface's top is kept short of
/// its cube (`cast::top`), so it stands on the top as drawn, not on its cube's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prop {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
    pub sink: u32,
}

impl Prop {
    pub const NONE: Prop = Prop {
        variant: 0,
        turn: 0,
        at: [0; 3],
        sink: 0,
    };
}

/// Where candidate `c` of `decor` stands at `cell_zoom`. It stands on its column's surface in
/// the world unplaced, the ground's alone, so no edit moves it or takes the ground from under
/// it, and it faces as the cells beside it vote (`asks`). Then, where `boxed`, it says whether
/// the candidate generates there: every cell of its box is void and every cell under its base
/// is a palette entry it generates on, as the ground holds them; no edit lies in its box, air
/// included (`DecorQuery::asset`, edited); and no placed decor's box overlaps its own
/// (`overlaps_placed`). It stands at its clip level's column surface, while the vote and its
/// box are asked at the finest zoom at which the world holds its leaf column
/// (`DecorQuery::finest`), around that zoom's column surface, so a candidate faces and
/// generates alike at every clip level it is generated at. Its pose stands as itself only at a
/// clip level its size stands at (`stands_at`). It returns where it stands, whether its column
/// surface is found, and whether it generates there, which is true where not `boxed` and it
/// stands.
pub fn placement(
    q: &mut impl DecorQuery,
    decor: &Decor,
    cell_zoom: Zoom,
    c: &Candidate,
    boxed: bool,
) -> (Prop, bool, bool) {
    let (f, ft, has_footing) = footing(q, cell_zoom, c.cell);
    if !has_footing {
        return (Prop::NONE, false, false);
    }
    let (prop, generates) = pose(q, &f, decor, cell_zoom, c.seed, &ft, boxed);
    (prop, true, generates)
}

/// Where a candidate in the leaf cell `leaf` stands at `cell_zoom` before it is posed
/// (`placement`): its box's min corner and sink, `Prop`'s with no pose; the finest zoom it is
/// posed at (`DecorQuery::finest`) and its column surface's top cell there, `base`; and
/// whether that zoom's column surface was found, `found`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footing {
    pub prop: Prop,
    pub zoom: Zoom,
    pub base: i32,
    pub found: bool,
}

/// The column surfaces under leaf cell `leaf` at `cell_zoom` (`placement`'s first half): its
/// clip level's, then the finest zoom's where that is another, through one ask, so a GPU's
/// walk down a column is written once. It returns the last surface asked, around which its
/// pose is asked, its `Footing`, and whether the clip level's surface was found; nothing
/// stands where it was not.
pub fn footing<Q: DecorQuery>(
    q: &mut Q,
    cell_zoom: Zoom,
    leaf: [i32; 2],
) -> (Q::ColumnSurface, Footing, bool) {
    let s = LEAF_CELL_ZOOM - cell_zoom;
    let zoom = q.finest(cell_zoom, leaf);
    let mut first = (0, 0.0);
    let mut k = 0;
    loop {
        let z = if k == 0 { cell_zoom } else { zoom };
        let t = LEAF_CELL_ZOOM - z;
        let (f, top, here) = q.column_surface(z, [leaf[0] >> t, leaf[1] >> t]);
        if k == 0 {
            first = top;
        }
        if (k == 0 && !here) || k == 1 || zoom == cell_zoom {
            let (floor, rise) = first;
            let ft = Footing {
                prop: Prop {
                    variant: 0,
                    turn: 0,
                    at: [leaf[0], leaf[1], (floor + 1) * (1 << s)],
                    sink: ((1.0 - rise) * (1u32 << (s + FRAC_W)) as f32 + 0.5) as u32,
                },
                zoom,
                base: top.0,
                found: k == 0 || here,
            };
            return (f, ft, k == 1 || here);
        }
        k += 1;
    }
}

/// How a candidate of `decor` with its `seed` stands on its footing `ft` at `cell_zoom`
/// (`placement`'s second half), its pose asked around the column surface `f` (`asks`), and
/// whether it generates there.
pub fn pose<Q: DecorQuery>(
    q: &mut Q,
    f: &Q::ColumnSurface,
    decor: &Decor,
    cell_zoom: Zoom,
    seed: u32,
    ft: &Footing,
    boxed: bool,
) -> (Prop, bool) {
    let (prop, generates) = asks(q, f, decor, (ft.zoom, ft.base), seed, ft.prop, boxed);
    (
        prop,
        ft.found && generates && stands_at(decor, prop.variant, cell_zoom),
    )
}

/// Every cell that generating decor asks of the world, at one place, since a GPU inlines a
/// call once for each place it is called from. A candidate is posed by its `seed`: first the
/// cells around its column surface that its pose votes on, then, where `boxed`, the box of `prop`
/// as that pose turned it (`placement`'s rule), each a cell at `zoom` around the column
/// surface `about`, whose top cell is `base`. The pose is the candidate's variant and its
/// heading in quarter turns, its seed's draws in order. Along transport, the heading is a
/// plurality vote of the cells a way runs on, by their offset from the candidate's along each
/// axis: a quarter turn if horizontal wins, else none (a tie or an isolated cell has no real
/// answer), and a half turn more on a draw, as two-way traffic faces both ways. The four at a
/// distance are asked in turn: before, after, under, over. It returns what stands, and
/// whether it generates there.
fn asks<Q: DecorQuery>(
    q: &mut Q,
    about: &Q::ColumnSurface,
    decor: &Decor,
    (zoom, base): (Zoom, i32),
    seed: u32,
    mut prop: Prop,
    boxed: bool,
) -> (Prop, bool) {
    let s = LEAF_CELL_ZOOM - zoom;
    let (mut flip, mut voting) = (0, 0);
    let mut draws = SplitMix32::new(seed);
    prop.variant = variant_of(decor, &mut draws);
    if decor.orientation == Orientation::along_transport {
        flip = draws.next_u32() >> 31;
        voting = 4 * ALIGN_NEIGHBOR_RADIUS;
    } else {
        prop.turn = draws.next_u32() >> (u32::BITS - TURN_BITS);
    }
    let foot = [prop.at[0] >> s, prop.at[1] >> s, base];
    let (mut horizontal, mut vertical) = (0, 0);
    // The box, laid out once the vote is in: its cells, from the one under
    // its base up, a column at a time.
    let (mut lo, mut n) = ([0; 3], [0; 3]);
    // Whether the box's cell asked last, void in the world unplaced, is asked again of the world
    // as edited, so that each cell's two asks go through the one ask.
    let mut edited = false;
    let mut ask = 0;
    loop {
        let voted = ask >= voting;
        let j = ask - voted as i32 * voting;
        if voted && j == 0 && !edited {
            // The vote is in: the turn it gives, then the box it turns.
            if voting > 0 {
                prop.turn = u32::from(horizontal > vertical) + 2 * flip;
            }
            if !boxed {
                break;
            }
            let [w, d, h] = decor.cells(prop.variant, prop.turn);
            let [x0, y0, _] = prop.at;
            lo = [x0 >> s, y0 >> s, base];
            let hi = [
                (x0 + w as i32 - 1) >> s,
                (y0 + d as i32 - 1) >> s,
                ((base + 1) * (1 << s) + h as i32 - 1) >> s,
            ];
            n = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        }
        if voted && j >= n[0] * n[1] * n[2] {
            break;
        }
        let b = if voted {
            [
                lo[0] + j / n[2] % n[0],
                lo[1] + j / (n[2] * n[0]),
                lo[2] + j % n[2],
            ]
        } else {
            let (d, side) = (ask / 4 + 1, ask % 4);
            let along = if side % 2 == 0 { -d } else { d };
            if side < 2 {
                [foot[0] + along, foot[1], foot[2]]
            } else {
                [foot[0], foot[1] + along, foot[2]]
            }
        };
        let v = q.asset(about, zoom, b, edited);
        if edited {
            // Void unplaced: it holds where no edit does.
            if answer::is_placed(v) {
                return (prop, false);
            }
            edited = false;
        } else if voted {
            // Under its base a palette entry it generates on; over it void,
            // and then no edit there, asked next.
            let holds = if b[2] == lo[2] {
                decor.allows(val(v))
            } else {
                v == value::VOID
            };
            if !holds {
                return (prop, false);
            }
            if b[2] != lo[2] {
                edited = true;
                continue;
            }
        } else if value::transports(val(v)) {
            if ask % 4 < 2 {
                horizontal += 1;
            } else {
                vertical += 1;
            }
        }
        ask += 1;
    }
    let [w, d, h] = decor.cells(prop.variant, prop.turn);
    (
        prop,
        !boxed || !overlaps_placed(q, prop.at, [w as i32, d as i32, h as i32]),
    )
}

/// How far decor standing at `at` (leaf cells) lies from `origin` (Q3 steps), in units,
/// squared: from its cell's middle at its base, with X wrapping around the world as the steps
/// do.
pub fn distance2(origin: [i32; 3], at: [i32; 3]) -> f32 {
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
/// `distance2`), given its record's zoom `cell_zoom` (`record_of`). One generated at a coarser
/// clip level is drawn where `at` is that clip level's zoom, so each distance draws one clip
/// level's decor; one generated at the leaves' is drawn where `at` is the leaf cell's zoom or
/// finer; and one `placed` is drawn where `at` is the coarsest zoom it stands at or finer, so
/// it stands where one of its size that the ground generates would. A generated one is asked
/// at the zoom held where it stands (`ray_zoom`), as a ray there draws. Each clip level
/// generates what stands as itself at it (pxl-core domain/view/lens.rs `decor_zooms`,
/// `stands_at`).
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
/// levels (`clipmap::clip_zoom`), or the finest coarser clip level that holds the column
/// present, as a ray reads it (`clipmap::step`), or the first where none does. Where a finer
/// clip level holds no brick, because the bake left an empty page out (open sea), a coarser
/// clip level's ground is drawn there, and its decor with it.
pub fn ray_zoom(w: &impl clipmap::Presence, at: Zoom, c: [i32; 2]) -> Zoom {
    let z = clipmap::clip_zoom(at);
    let s = LEAF_CELL_ZOOM - z;
    let g = [wrap_idx(z, c[0] >> s), c[1] >> s];
    let (f, _) = clipmap::step(w, clipmap::Lookup::NONE, z, g);
    if at < f.from() {
        at
    } else {
        f.from()
    }
}

/// Placed decor's tag (`node_seed`).
pub const PLACED_TAG: u32 = 0x91ac_ed5e;

/// A placed decor's variant and heading, seeded only by the unit cell of its
/// corner, its one stable identity, so a preview predicts exactly what a
/// placement gets.
pub fn placed_pose(decor: &Decor, corner: [i32; 3]) -> (u32, u32) {
    let mut draws = SplitMix32::new(node_seed(PLACED_TAG, (UNIT_ZOOM, corner)));
    let variant = variant_of(decor, &mut draws);
    (variant, draws.next_u32() >> (u32::BITS - TURN_BITS))
}

/// The row of `rows` a stored value names; false for one not decor.
pub fn row_of(rows: &[Decor], val: u32) -> (u32, bool) {
    let mut k = 0;
    while k < rows.len() {
        if rows[k].val == val {
            return (k as u32, true);
        }
        k += 1;
    }
    (0, false)
}

/// How far before a box's min corner, in unit cells, a placed decor's corner may lie with
/// its box still meeting the box: any decor's widest side and tallest box, less one.
pub const CLAIM_EXTENT: [i32; 3] = {
    let (mut wide, mut tall, mut k) = (1, 1, 0);
    while k < DECORS.len() {
        let d = &DECORS[k];
        wide = if d.widest() > wide { d.widest() } else { wide };
        let mut v = 0;
        while v < d.variants as usize {
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
        let (k, decor) = row_of(q.rows(), word & 0xffff);
        if decor {
            let row = &q.rows()[k as usize];
            let (variant, turn) = placed_pose(row, c);
            let [w, e, v] = row.cells(variant, turn);
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

impl Decor {
    /// Its records' room in a brick, a whole number of runs (`RUN`): twice the mean it retains at
    /// the coarsest clip level a variant stands at (`scatter::root`), where a brick takes the most
    /// leaf cells, plus two runs.
    pub const fn cap(&self) -> u32 {
        let root = root(self);
        let cell_zoom = if root > FIRST_CELL_ZOOM {
            root
        } else {
            FIRST_CELL_ZOOM
        };
        let side = BRICK << (LEAF_CELL_ZOOM - cell_zoom);
        let cells = if side < 1 << 16 {
            side * side
        } else {
            u32::MAX
        };
        (2 * mul_hi(cells, self.chance) + 2 * RUN).div_ceil(RUN) * RUN
    }
}

#[cfg(test)]
#[path = "generation.test.rs"]
mod test;
