// pxl-spec-core/generated.rs

//! gpu-portable: the decor the ground holds (`scatter`) generated on the
//! world as either side holds it: where a candidate stands, which way it
//! faces and whether its footing holds (`generated`, over `DecorQuery`), what
//! placed decor claims (`overlaps_placed`), and how the GPU lays, lists and
//! draws what it generates in a window's bricks.

use xpute_kit::math::rng::SplitMix32;

use crate::answer::val;
use crate::cast::Detail;
use crate::cave::node_seed;
use crate::cube::{sentinel, Ground};
use crate::decor::{Decor, Orientation, DECORS};
use crate::overwrite;
use crate::param::scatter::ALIGN_NEIGHBOR_RADIUS;
use crate::scatter::{mul_hi, stands_at, variant_of, Candidate};
use crate::window;
use crate::window::{BRICK, BRICK_SHIFT, ENTRIES, EYE_WINDOWS, FIRST_CELL};
use crate::world::{placed_in, Placed};
use crate::{wrap_idx, Zoom, FRAC_W, LEAF_CELL, UNIT_ZOOM};

/// The records a run of a decor pass reads (pxl-shader decor.rs `run_of`),
/// a decor's capacity a whole number of them.
pub const RUN: u32 = 16;

/// The most decor one brick generates, every decor's together: each decor's
/// capacity (`Decor::cap`) laid in the catalog's order.
pub const BRICK_RECORDS: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        sum += DECORS[k].cap();
        k += 1;
    }
    sum
};

/// The most records one decor holds in a brick, every decor's
/// (`Decor::cap`): what `decor_cs` is dispatched over.
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
/// it (`Generated::sink`).
pub const RECORD_WORDS: u32 = 5;

/// The eye's windows' bricks, an entry each: what the GPU generates decor in,
/// entry `s` the brick the windows' table holds at `s`.
pub const EYE_ENTRIES: u32 = EYE_WINDOWS * ENTRIES;

/// A record no decor stands at: its candidate is deleted by the thinning or
/// generates nothing where it is, so it is drawn as nothing (`drawn::NONE`
/// its region).
pub const UNGENERATED: u32 = u32::MAX;
const _: () = assert!(UNGENERATED >> drawn::REGION_SHIFT == drawn::NONE);
/// A record retained by the thinning and still to generate: `decor_thin_cs`
/// lays it, its candidate's cell and seed after it, and `decor_cs`
/// generates it in its place the same frame, so no other pass reads one.
pub const RETAINED: u32 = UNGENERATED - 1;
const _: () = assert!(
    crate::FINEST_ZOOM < (1 << crate::ZOOM_BITS) - 1,
    "no record's first word is `RETAINED` or `UNGENERATED`: its zoom's bits are never all set"
);

/// The GPU's decor in one buffer of words: a head per eye entry, then each
/// entry's `BRICK_RECORDS` records, then placed decor's head and its records.
/// An entry's records are each decor's in turn, as many as it retained
/// (`entry_record`), a decor generated again laying its own anew. A head is
/// the brick they were generated for, X and Y, and 1 once they were — an
/// entry whose brick the windows no longer hold there is skipped by it
/// (`generated_for`) — then how many records each decor laid (`cnt_at`). Placed decor's head is its
/// count.
pub const CNT_AT: u32 = 3;
pub const HEAD_WORDS: u32 = CNT_AT + DECORS.len() as u32;
const SLOTS_AT: u32 = EYE_ENTRIES * HEAD_WORDS;
/// Records the entries hold, every one, ahead of placed decor's.
pub const SLOT_RECORDS: u32 = EYE_ENTRIES * BRICK_RECORDS;
pub const PLACED_HEAD: u32 = SLOTS_AT + SLOT_RECORDS * RECORD_WORDS;
pub const PLACED_HEAD_WORDS: u32 = 4;

/// The runs of records a decor pass's workgroup reads (pxl-shader decor.rs
/// `run_of`): an entry's, `RUN` records each; placed decor's, a record each.
pub const RUNS: u32 = BRICK_RECORDS / RUN;

/// Entry `s`'s head.
pub const fn head_at(s: u32) -> u32 {
    s * HEAD_WORDS
}

/// The word of entry `s`'s head counting the records decor `k` laid.
pub const fn cnt_at(s: u32, k: u32) -> u32 {
    head_at(s) + CNT_AT + k
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

/// Record `n`'s first word, the entries' first, entry by entry, then placed
/// decor's.
pub const fn record_at(n: u32) -> u32 {
    if n < SLOT_RECORDS {
        SLOTS_AT + n * RECORD_WORDS
    } else {
        PLACED_HEAD + PLACED_HEAD_WORDS + (n - SLOT_RECORDS) * RECORD_WORDS
    }
}

/// Whether an entry's head says its records were generated for the brick `q`.
pub const fn generated_for(head: [u32; 3], q: [i32; 2]) -> bool {
    head[2] == 1 && head[0] as i32 == q[0] && head[1] as i32 == q[1]
}

/// The boxes a frame's generation is asked over at most: the edits and cuts
/// held (`overwrite::BOXES`), and each brick handed this frame, which
/// reaches its window's zoom alone.
pub const MARKED: u32 = overwrite::BOXES + window::HANDED;
/// A box's words: its leaf cells `[x0, y0, x1, y1)`, X on any lap, then the
/// window zoom whose candidates it reaches, `EVERY` for all.
pub const BOX_WORDS: u32 = 5;
pub const EVERY: u32 = 0;
const _: () = assert!(FIRST_CELL > EVERY);

/// What `decor_mark_cs` reads and lists and `decor_thin_cs` and `decor_cs`
/// are dispatched by, in words: each one's workgroups across, up and the
/// entry-decor pairs listed, a pad (`THIN_ARGS`, `GENERATE_ARGS`); whether
/// every entry generates whole, decor having been off (`LIST_EVERY`), and how
/// many boxes (`LIST_BOXES`), two pads; the boxes, `BOX_WORDS` each; then the
/// list, an entry, its decor (`<< 16`) and whether it generates whole
/// (`WHOLE`) each, in no order.
pub const THIN_ARGS: u32 = 0;
pub const GENERATE_ARGS: u32 = 4;
pub const LIST_EVERY: u32 = 8;
pub const LIST_BOXES: u32 = 9;
pub const BOXES_AT: u32 = 12;
pub const LIST_AT: u32 = BOXES_AT + MARKED * BOX_WORDS;
pub const WHOLE: u32 = 1 << 31;

/// The words of what `decor_mark_cs` lists.
pub const LIST_WORDS: u32 = LIST_AT + EYE_ENTRIES * DECORS.len() as u32;

/// How far about its own, in its window's cells, a candidate's generation
/// reads the world (`generated`): the cells its pose votes on, its box, and the placed
/// decor that may claim it, none wider than the widest decor.
const READ_REACH: i32 = ALIGN_NEIGHBOR_RADIUS + WIDEST;

/// The leaf cells, `[x0, y0, x1, y1)`, of brick `q` of the window at `cell`.
pub const fn brick_box(cell: Zoom, q: [i32; 2]) -> [i32; 4] {
    let k = BRICK_SHIFT + LEAF_CELL - cell;
    [q[0] << k, q[1] << k, (q[0] + 1) << k, (q[1] + 1) << k]
}

/// The leaf cells any candidate in brick `q` of the window at `cell` reads:
/// its window's cells within `READ_REACH` of the brick.
pub const fn brick_reads(cell: Zoom, q: [i32; 2]) -> [i32; 4] {
    let ([x0, y0, x1, y1], r) = (brick_box(cell, q), READ_REACH << (LEAF_CELL - cell));
    [x0 - r, y0 - r, x1 + r, y1 + r]
}

/// Whether the box `b` of leaf cells meets the leaf cells `reads`
/// (`reads`, `brick_reads`), each X on any lap, X round the world.
pub fn meets(reads: [i32; 4], b: [i32; 4]) -> bool {
    let wide = reads[2] - reads[0];
    // The box's first column from the region's, the short way round.
    let (from, width) = (wrap_idx(LEAF_CELL, b[0] - reads[0]), b[2] - b[0]);
    // Met where it starts in the region or over it, or runs on round the
    // world into it.
    let across = width >= 1 << LEAF_CELL
        || (from < wide && from + width > 0)
        || from + width > 1 << LEAF_CELL;
    across && b[1] < reads[3] && b[3] > reads[1]
}

/// A record's first word: its decor and variant, a byte each, its turn in
/// quarters, and the window zoom it was generated at, or for placed decor
/// the coarsest it stands at (`scatter::coarsest_stand`); past them, once
/// sized, its region (`drawn::REGION_SHIFT`).
pub const fn record_word(decor: u32, variant: u32, turn: u32, cell: Zoom) -> u32 {
    decor | variant << 8 | turn << TURN_SHIFT | cell << CELL_SHIFT
}

/// A record's turn's place and width in its first word, and its zoom's
/// place past it (`record_word`).
const TURN_SHIFT: u32 = 16;
const TURN_BITS: u32 = 2;
const CELL_SHIFT: u32 = TURN_SHIFT + TURN_BITS;

/// A record's decor, variant, turn and zoom (`record_word`).
pub const fn record_of(w: u32) -> (u32, u32, u32, Zoom) {
    (
        w & 0xff,
        (w >> 8) & 0xff,
        (w >> TURN_SHIFT) & ((1 << TURN_BITS) - 1),
        (w >> CELL_SHIFT) & ((1 << crate::ZOOM_BITS) - 1),
    )
}

/// Every variant of every decor, numbered decor by decor in the catalog's
/// order: a decor's draw group (`first_group`).
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

/// How the GPU draws what it generates: each record sized for the screen, a
/// model or an impostor, into its group's region, models at `2·group` and
/// impostors past it; the regions laid end to end in the decor's instances, in the
/// layout the decor and impostor shaders read (lanes x, y, z, spin, scale,
/// `drawn::LANES` a slot, then the index list), each region's draw a set of
/// indirect args.
pub mod drawn {
    use super::GROUPS;

    pub const REGIONS: u32 = 2 * GROUPS;
    /// A record drawn as nothing.
    pub const NONE: u32 = 0xff;
    /// What a record's first word holds past its decor, variant and turn:
    /// its region, once sized.
    pub const REGION_SHIFT: u32 = super::CELL_SHIFT + crate::ZOOM_BITS;
    pub const LANES: u32 = 5;
    /// Each region's indirect args, `ARGS_WORDS` a region: a model's
    /// indexed, an impostor's a strip of four corners an instance; its first
    /// instance 0, which a device without `indirect-first-instance` holds
    /// every indirect draw to.
    pub const ARGS_WORDS: u32 = 5;
    /// Each region's draw uniform, `UNIFORM_WORDS` apart (`OFF_BYTES`, every
    /// device's dynamic offset alignment), its index list's first slot at
    /// `UNIFORM_IDX` within it: the lanes' base, to which the args pass adds
    /// the region's first slot once the regions are laid out.
    pub const UNIFORM_WORDS: u32 = OFF_BYTES / 4;
    /// The dynamic offset alignment every WebGPU device meets: its
    /// `minUniformBufferOffsetAlignment` is 256 bytes at most.
    const OFF_BYTES: u32 = 256;
    pub const UNIFORM_IDX: u32 = 7;
    /// The drawing words, what a frame's decor is drawn by: the model's and
    /// the impostor's floors, each an angle about the eye in radians, the
    /// anchor the lanes' positions are about (unit cells, X wrapping), the
    /// left-out decor's corner and whether there is one, then each group's
    /// extent.
    pub const MODEL_ANGLE: u32 = 0;
    pub const IMPOSTOR_ANGLE: u32 = 1;
    pub const ANCHOR: u32 = 4;
    pub const HIDE: u32 = 8;
    pub const EXTENTS: u32 = 12;
    pub const DRAWING_WORDS: u32 = EXTENTS + GROUPS;

    /// What a thing spanning `across` radians about the eye is drawn as: a
    /// model at `model` or more, else an impostor at `impostor` or more, else
    /// culled, nothing drawn, each an angle, so no screen decides it. A
    /// group's region is `2·group` past its tier where drawn.
    pub const MODEL: u32 = 0;
    pub const IMPOSTOR: u32 = 1;
    pub const CULLED: u32 = 2;

    /// The tier of what spans `across` radians, against the model's and the
    /// impostor's floors: the one rule decor's count and the CPU's clouds
    /// both size by.
    pub fn tier(across: f32, model: f32, impostor: f32) -> u32 {
        if across >= model {
            MODEL
        } else if across >= impostor {
            IMPOSTOR
        } else {
            CULLED
        }
    }
}

/// What generating decor asks of the world, as either side holds it: of the
/// world unplaced (`world::Handed::unplaced`), the ground's alone, its column's
/// footing and the assets about it; of the world as edited, whether an edit holds a
/// cell and what is placed in a box; and the catalog's rows where it reads
/// them.
pub trait DecorQuery {
    /// A footing as this side keeps it: what its assets are asked off.
    type Footing;
    /// The catalog's rows (`decor::DECORS`), as this side holds them: a GPU
    /// reads them from a buffer, never a table it copies.
    fn rows(&self) -> &[Decor];
    /// The footing down column `g` at `cell` of the world unplaced,
    /// what is placed left out: the query's support (`support::footing`) as
    /// this side asks it; its top (`cast::top`), false where nothing fills
    /// there or the world lacks it.
    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (Self::Footing, (i32, f32), bool);
    /// What is in the cell `c` of `zoom` about the footing `f`, the world
    /// unplaced's answer (`support::asset`): `void` or the ground's value.
    fn asset(&mut self, f: &Self::Footing, zoom: Zoom, c: [i32; 3]) -> u32;
    /// Whether an edit holds the cell `c` of `zoom` in the world as edited,
    /// something placed or air (`answer::is_placed`).
    fn is_edited(&mut self, zoom: Zoom, c: [i32; 3]) -> bool;
    /// The next leaf placed in `p`'s box, its cell and its word, false past
    /// the last (`world::Placed::next`).
    fn next_placed(&self, p: &mut Placed) -> ([i32; 3], u32, bool);
}

/// Decor the ground generates, where it stands: its variant, its heading in
/// quarter turns, its box's min corner, in leaf cells, the cell over its
/// footing's, and how far its base lies under that corner, in Q3 steps: the
/// footing's top kept short of its cube (`cast::top`), so it stands on the
/// top as drawn, not its cube's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generated {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
    pub sink: u32,
}

impl Generated {
    pub const NONE: Generated = Generated {
        variant: 0,
        turn: 0,
        at: [0; 3],
        sink: 0,
    };
}

/// Where candidate `c` of `decor` stands at `cell`: on its column's footing
/// in the world unplaced, the ground's alone, so no edit moves
/// it or takes it from under it; facing as the cells beside it at the
/// footing's level vote there (`asks`); then, where `boxed`, whether it
/// generates there: every cell of its box void and every one under its base
/// a look it generates on, as the ground holds them; no edit in its box, air
/// included (`DecorQuery::is_edited`); and no placed decor's box overlaps
/// its own (`overlaps_placed`). Every cell both ask is asked at one place
/// about one footing (`asks`). Its pose stands as itself only at a window its
/// size does (`stands_at`). Where it stands, whether its footing is found,
/// and whether it generates there, true where not `boxed` and it stands.
pub fn generated(
    q: &mut impl DecorQuery,
    decor: &Decor,
    cell: Zoom,
    c: &Candidate,
    boxed: bool,
) -> (Generated, bool, bool) {
    let s = LEAF_CELL - cell;
    let at = [c.cell[0] >> s, c.cell[1] >> s];
    let (f, (floor, rise), here) = q.footing(cell, at);
    if !here {
        return (Generated::NONE, false, false);
    }
    let g = Generated {
        variant: 0,
        turn: 0,
        at: [c.cell[0], c.cell[1], (floor + 1) * (1 << s)],
        sink: ((1.0 - rise) * (1u32 << (s + FRAC_W)) as f32 + 0.5) as u32,
    };
    let (g, generates) = asks(q, &f, decor, cell, c.seed, g, boxed);
    (g, true, generates && stands_at(decor, g.variant, cell))
}

/// Every cell generating decor asks of the world, at one place, as a GPU
/// inlines a call once a place it is called from: a candidate posed by its
/// `seed`, the cells about its footing its pose votes on, then, where
/// `boxed`, the box of `g` as that pose turned it (`generated`'s rule).
/// The pose is the candidate's variant and its heading in quarter turns,
/// its seed's draws in order: along transport, a plurality vote of the
/// cells a way runs on, by their offset from the candidate's, along each
/// axis — a quarter turn if horizontal wins, else none (a tie or isolated
/// cell has no real answer), and a half more on a draw, as two-way traffic
/// faces both ways — the four at a distance in turn, before, after, under,
/// over. What stands, and whether it generates there.
fn asks<Q: DecorQuery>(
    q: &mut Q,
    f: &Q::Footing,
    decor: &Decor,
    cell: Zoom,
    seed: u32,
    mut g: Generated,
    boxed: bool,
) -> (Generated, bool) {
    let s = LEAF_CELL - cell;
    let (mut flip, mut voting) = (0, 0);
    let mut draws = SplitMix32::new(seed);
    g.variant = variant_of(decor, &mut draws);
    if decor.orientation == Orientation::along_transport as u32 {
        flip = draws.next_u32() >> 31;
        voting = 4 * ALIGN_NEIGHBOR_RADIUS;
    } else {
        g.turn = draws.next_u32() >> (u32::BITS - TURN_BITS);
    }
    let foot = [g.at[0] >> s, g.at[1] >> s, (g.at[2] >> s) - 1];
    let (mut horizontal, mut vertical) = (0, 0);
    // The box, laid out once the vote is in: its cells, from the one under
    // its base up, a column at a time.
    let (mut lo, mut n) = ([0; 3], [0; 3]);
    let mut ask = 0;
    loop {
        let voted = ask >= voting;
        let j = ask - voted as i32 * voting;
        if voted && j == 0 {
            // The vote is in: the turn it gives, then the box it turns.
            if voting > 0 {
                g.turn = u32::from(horizontal > vertical) + 2 * flip;
            }
            if !boxed {
                break;
            }
            let [w, d, h] = decor.cells(g.variant, g.turn);
            let [x0, y0, z] = g.at;
            lo = [x0 >> s, y0 >> s, (z - 1) >> s];
            let hi = [
                (x0 + w as i32 - 1) >> s,
                (y0 + d as i32 - 1) >> s,
                (z + h as i32 - 1) >> s,
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
        let v = q.asset(f, cell, b);
        if voted {
            let holds = if b[2] == lo[2] {
                decor.allows(val(v))
            } else {
                v == sentinel::void as u32 && !q.is_edited(cell, b)
            };
            if !holds {
                return (g, false);
            }
        } else if Ground::transports(val(v)) {
            if ask % 4 < 2 {
                horizontal += 1;
            } else {
                vertical += 1;
            }
        }
        ask += 1;
    }
    let [w, d, h] = decor.cells(g.variant, g.turn);
    (
        g,
        !boxed || !overlaps_placed(q, g.at, [w as i32, d as i32, h as i32]),
    )
}

/// How far decor standing at `at` (leaf cells) lies from `origin` (Q3
/// steps), in units, squared: from its cell's middle at its base, X round
/// the world as the steps wrap.
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

/// Whether decor `d2` from the eye, its distance squared (`distance2`), is
/// drawn, its record's zoom `cell` (`record_of`): one generated at a coarser
/// window where the rays ask that window's zoom at its distance (`Detail::at`),
/// so each distance draws one window's, and one at the leaves' where they ask
/// the leaf cell's zoom or finer; one `placed` where they ask the coarsest it
/// stands at or finer, so it stands where one the ground generates of its
/// size would. Each window generates what stands as itself at it (pxl-kernel
/// domain/scene/decor/drawn.rs `shown`, `stands_at`).
pub fn drawn_at(detail: Detail, d2: f32, cell: Zoom, placed: bool) -> bool {
    let at = detail.at(d2);
    if placed {
        at >= cell
    } else if cell >= LEAF_CELL {
        at >= LEAF_CELL
    } else {
        at == cell
    }
}

/// Placed decor's tag (`node_seed`).
const PLACED_TAG: u32 = 0x91ac_ed5e;

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

/// How far before a box's min corner, in unit cells, a placed decor's corner
/// may lie and its box still meet it: any decor's widest side and tallest
/// box, less one.
const CLAIM_REACH: [i32; 3] = {
    let (mut wide, mut tall, mut k) = (1, 1, 0);
    while k < DECORS.len() {
        let d = &DECORS[k];
        assert!(
            d.scale_steps == 0,
            "decor is generated and claimed in unit cells"
        );
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

/// The tallest decor and the widest, in unit cells: as high over what it
/// stands on as anything generated reaches, and as far across (pxl-spec-core
/// light.rs `Light::up`, `Light::ring`).
pub const TALLEST: i32 = CLAIM_REACH[2] + 1;
pub const WIDEST: i32 = CLAIM_REACH[0] + 1;

/// Whether a placed decor's box overlaps the box from `at` (unit cells),
/// `cells` a side: one whose corner, posed as placed (`placed_pose`), holds
/// a box meeting it, each placed leaf near enough asked of the world's
/// (`DecorQuery::next_placed`).
pub fn overlaps_placed(q: &impl DecorQuery, at: [i32; 3], cells: [i32; 3]) -> bool {
    let lo = [
        at[0] - CLAIM_REACH[0],
        at[1] - CLAIM_REACH[1],
        at[2] - CLAIM_REACH[2],
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
    /// Its records' room in a brick, a whole number of runs (`RUN`): twice the
    /// mean it retains at the coarsest window a variant stands at, where a
    /// brick takes the most leaf cells, and two runs more.
    pub const fn cap(&self) -> u32 {
        // The coarsest window a variant stands at, the most leaf cells a
        // brick there takes.
        let mut cell = FIRST_CELL;
        'coarsest: while cell < LEAF_CELL {
            let mut v = 0;
            while v < self.variants {
                if stands_at(self, v, cell) {
                    break 'coarsest;
                }
                v += 1;
            }
            cell += 1;
        }
        let side = BRICK << (LEAF_CELL - cell);
        let cells = if side < 1 << 16 {
            side * side
        } else {
            u32::MAX
        };
        (2 * mul_hi(cells, self.chance) + 2 * RUN).div_ceil(RUN) * RUN
    }
}

#[cfg(test)]
#[path = "generated.test.rs"]
mod test;
