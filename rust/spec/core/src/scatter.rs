// pxl-spec-core/scatter.rs

//! gpu-portable: decor the ground grows on its own, never stored. A Poisson
//! process on the leaf cell lattice, never a grid: a patch at zoom m bears a
//! count of candidates born there, each at a uniform leaf cell, and a
//! candidate born at m stands at every zoom from m finer, so a cell at any
//! zoom holds the decor's density. Every draw is a u32 off one stream a patch
//! (xpute-kit `SplitMix32`), seeded by the decor's seed and the patch
//! (`node_seed`), so every reader of a cell, either side, draws the same.
//!
//! Where a candidate stands, which way it faces and whether its footing
//! holds are functions of the world as either side holds it (`grown`, over
//! `Grounds`), and so is what placed decor claims (`claimed`).

use xpute_kit::math::rng::{derive_seed_u32, SplitMix32};

use crate::answer::value;
use crate::cast::Detail;
use crate::cave::node_seed;
use crate::cube::{sentinel, Ground};
use crate::decor::{Counts, Decor, Orientation, COUNTS, DECORS, PATCH};
use crate::lattice::GROUND;
use crate::world::{placed_in, Placed};
use crate::window::{EYE_WINDOWS, SLOTS, SQUARE_SHIFT};
use crate::{wrap_index, Zoom, FRAC_W, LEAF_CELL, UNIT_ZOOM};
use crate::param::tile::{LEAF_ZOOM, TOP_ZOOM};
use crate::param::scatter::ALIGN_NEIGHBOR_RADIUS;

const PATCH_SHIFT: u32 = PATCH.trailing_zeros();
const _: () = assert!(
    PATCH.is_power_of_two(),
    "a patch's side is a power of two, so a uniform cell in it is a draw's top bits"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The leaf cell it stands at, at every zoom.
    pub cell: [i32; 2],
    /// The coarsest zoom it stands at.
    pub born: Zoom,
    /// Its own draws: variant, heading.
    pub seed: u32,
}

impl Candidate {
    pub const NONE: Candidate = Candidate {
        cell: [0; 2],
        born: 0,
        seed: 0,
    };
}

impl Counts {
    /// The count a draw `u` bears: how many of its thresholds `u` reaches,
    /// each the cumulative Poisson probability of one more, over 2^32.
    pub fn of(&self, u: u32) -> u32 {
        let mut k = 0;
        while k < self.len && u >= self.at[k as usize] {
            k += 1;
        }
        k
    }
}

/// The patch of `decor`'s process at `born` with index `b`: its seed, and its
/// side in leaf cells as a shift.
fn patch_seed(decor: &Decor, born: Zoom, b: [i32; 2]) -> (u32, u32) {
    (
        node_seed(decor.seed, (born, b)),
        PATCH_SHIFT + GROUND.finest_in(born).trailing_zeros(),
    )
}

/// Each candidate the patch of `decor` at `born` with index `b` bears, in
/// its stream's order, handed to `each` with its index; how many it bears.
/// The one reading of a patch's stream.
pub fn each_in_patch(
    decor: &Decor,
    born: Zoom,
    b: [i32; 2],
    mut each: impl FnMut(u32, Candidate),
) -> u32 {
    let (seed, shift) = patch_seed(decor, born, b);
    let mut draws = SplitMix32::new(seed);
    let n = decor.counts[usize::from(born != TOP_ZOOM)].of(draws.next_u32());
    let mut i = 0;
    while i < n {
        let x = (b[0] << shift) + (draws.next_u32() >> (32 - shift)) as i32;
        let y = (b[1] << shift) + (draws.next_u32() >> (32 - shift)) as i32;
        each(
            i,
            Candidate {
                cell: [x, y],
                born,
                seed: derive_seed_u32(seed, i),
            },
        );
        i += 1;
    }
    n
}

/// Every candidate of `decor` born at `zoom` or coarser in `[min, max)`
/// (leaf cells), patch by patch, coarsest first.
pub fn each_candidate(
    decor: &Decor,
    zoom: Zoom,
    [min, max]: [[i32; 2]; 2],
    mut each: impl FnMut(Candidate),
) {
    let mut born = zoom + 1;
    while born > TOP_ZOOM {
        born -= 1;
        let (_, shift) = patch_seed(decor, born, [0, 0]);
        let mut by = min[1] >> shift;
        while by <= (max[1] - 1) >> shift {
            let mut bx = min[0] >> shift;
            while bx <= (max[0] - 1) >> shift {
                each_in_patch(decor, born, [bx, by], |_, c| {
                    let [x, y] = c.cell;
                    if min[0] <= x && x < max[0] && min[1] <= y && y < max[1] {
                        each(c);
                    }
                });
                bx += 1;
            }
            by += 1;
        }
    }
}

/// The `n`-th patch meeting `[min, max)` (leaf cells) of those born at
/// `zoom` or coarser, the finest born first: its zoom and index; false past
/// the last.
pub fn patch_of(zoom: Zoom, [min, max]: [[i32; 2]; 2], n: u32) -> (Zoom, [i32; 2], bool) {
    let top = TOP_ZOOM;
    let (mut born, mut n) = (zoom + 1, n);
    while born > top {
        born -= 1;
        let shift = PATCH_SHIFT + GROUND.finest_in(born).trailing_zeros();
        let first = [min[0] >> shift, min[1] >> shift];
        let across = [
            ((max[0] - 1) >> shift) - first[0] + 1,
            ((max[1] - 1) >> shift) - first[1] + 1,
        ];
        let count = (across[0] * across[1]) as u32;
        if n < count {
            return (
                born,
                [
                    first[0] + (n % across[0] as u32) as i32,
                    first[1] + (n / across[0] as u32) as i32,
                ],
                true,
            );
        }
        n -= count;
    }
    (top, [0, 0], false)
}

/// The most patches `patch_of` names over a window's square at any zoom: a
/// square at the leaves, its side `window::SQUARE` leaf cells, aligned to
/// it, so a patch as wide or wider holds it whole.
pub const SQUARE_PATCHES: u32 = {
    let side = crate::window::SQUARE_SHIFT;
    let (mut born, mut sum) = (LEAF_ZOOM + 1, 0);
    while born > TOP_ZOOM {
        born -= 1;
        let shift = PATCH_SHIFT + GROUND.finest_in(born).trailing_zeros();
        let across = if side > shift { 1 << (side - shift) } else { 1 };
        sum += across * across;
    }
    sum
};

/// The most decor one square grows, every decor's together: a patch bears no more
/// than its counts' thresholds (`Decor::room`), and no more patches than
/// `SQUARE_PATCHES` meet a square.
pub const SQUARE_GROWN: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        sum += DECORS[k].room();
        k += 1;
    }
    sum * SQUARE_PATCHES
};

const _: () = assert!(
    size_of::<Decor>().is_multiple_of(4),
    "a row is whole words, as a GPU reads the table"
);

/// A record of decor the GPU draws: its first word (`record_word`), then
/// its box's min corner in leaf cells, then how far its base sinks under
/// it (`Grown::sink`).
pub const GROWN_WORDS: u32 = 5;

/// The eye's windows' squares, a slot each: what the GPU grows decor in,
/// slot `s` the square the windows' table holds at `s`.
pub const EYE_SLOTS: u32 = EYE_WINDOWS * SLOTS;

/// A record no decor stands at: its candidate stands or grows not where it
/// is, so it is drawn as nothing (`drawn::NONE` its region).
pub const UNGROWN: u32 = u32::MAX;
const _: () = assert!(UNGROWN >> drawn::REGION_SHIFT == drawn::NONE);
/// A record stood and still to grow: `decor_stand_cs` lays it, its
/// candidate's cell, seed and born zoom after it, and `decor_cs` grows it
/// in its place the same frame, so no other pass reads one.
pub const STOOD: u32 = UNGROWN - 1;
const _: () = assert!(
    crate::FINEST_ZOOM < (1 << crate::ZOOM_BITS) - 1,
    "no record's first word is `STOOD` or `UNGROWN`: its zoom's bits are never all set"
);

/// The GPU's decor in one buffer of words: a head per eye slot, then each
/// slot's `SQUARE_GROWN` records, then placed decor's head and its records.
/// A slot's records are its candidates', each at its own place
/// (`slot_record`), so one grown again rewrites its own alone. A head is
/// the square they were grown for, X and Y, and 1 once they were — a slot
/// whose square the windows no longer hold there is skipped by it
/// (`grown_for`) — then how many candidates each patch of each decor bears,
/// as many of its records as are read (`bears_at`). Placed decor's head is
/// its count.
pub const BEARS_AT: u32 = 3;
pub const HEAD_WORDS: u32 = BEARS_AT + RUNS;
const SLOTS_AT: u32 = EYE_SLOTS * HEAD_WORDS;
/// Records the slots hold, every one, ahead of placed decor's.
pub const SLOT_RECORDS: u32 = EYE_SLOTS * SQUARE_GROWN;
pub const PLACED_HEAD: u32 = SLOTS_AT + SLOT_RECORDS * GROWN_WORDS;
pub const PLACED_HEAD_WORDS: u32 = 4;

/// The runs of records a decor pass's workgroup reads (pxl-shader decor.rs
/// `records`): a slot's, each decor's patches in turn; placed decor's, a
/// record each.
pub const RUNS: u32 = DECORS.len() as u32 * SQUARE_PATCHES;

/// Slot `s`'s head.
pub const fn head_at(s: u32) -> u32 {
    s * HEAD_WORDS
}

/// The word of slot `s`'s head counting what the `p`-th patch of decor `k`
/// bears.
pub const fn bears_at(s: u32, k: u32, p: u32) -> u32 {
    head_at(s) + BEARS_AT + k * SQUARE_PATCHES + p
}

/// Slot `s`'s record of candidate `i` of the `p`-th patch of `rows`' decor
/// `k`, as a record number (`record_at`): the decors' rooms laid in the
/// catalog's order, a patch's room each (`Decor::room`).
pub fn slot_record(rows: &[Decor], s: u32, k: u32, p: u32, i: u32) -> u32 {
    let (mut first, mut j) = (0, 0);
    while j < k {
        first += rows[j as usize].room();
        j += 1;
    }
    s * SQUARE_GROWN + first * SQUARE_PATCHES + p * rows[k as usize].room() + i
}

/// Record `n`'s first word, the slots' first, slot by slot, then placed
/// decor's.
pub const fn record_at(n: u32) -> u32 {
    if n < SLOT_RECORDS {
        SLOTS_AT + n * GROWN_WORDS
    } else {
        PLACED_HEAD + PLACED_HEAD_WORDS + (n - SLOT_RECORDS) * GROWN_WORDS
    }
}

/// Whether a slot's head says its records were grown for the square `q`.
pub const fn grown_for(head: [u32; 3], q: [i32; 2]) -> bool {
    head[2] == 1 && head[0] as i32 == q[0] && head[1] as i32 == q[1]
}

/// The boxes a frame's growth is asked over at most: the edits and cuts
/// held (`overwrite::BOXES`), and each square handed this frame, which
/// reaches its window's zoom alone.
pub const MARKED: u32 = crate::overwrite::BOXES + crate::window::HANDED;
/// A box's words: its leaf cells `[x0, y0, x1, y1)`, X on any lap, then the
/// window zoom whose candidates it reaches, `EVERY` for all.
pub const BOX_WORDS: u32 = 5;
pub const EVERY: u32 = 0;
const _: () = assert!(crate::window::FIRST_CELL > EVERY);

/// What `decor_mark_cs` reads and lists and `decor_stand_cs` and `decor_cs`
/// are dispatched by, in words: each one's workgroups across, up and the
/// slot-decor pairs listed, a pad (`STAND_ARGS`, `GROW_ARGS`); whether every
/// slot grows whole, decor having been off (`GROWS_EVERY`), and how many
/// boxes (`GROWS_BOXES`), two pads; the boxes, `BOX_WORDS` each; then the
/// list, a slot, its decor (`<< 16`) and whether it grows whole (`WHOLE`)
/// each, in no order.
pub const STAND_ARGS: u32 = 0;
pub const GROW_ARGS: u32 = 4;
pub const GROWS_EVERY: u32 = 8;
pub const GROWS_BOXES: u32 = 9;
pub const BOXES_AT: u32 = 12;
pub const GROWS_AT: u32 = BOXES_AT + MARKED * BOX_WORDS;
pub const WHOLE: u32 = 1 << 31;

/// The words of what `decor_mark_cs` lists.
pub const GROWS_WORDS: u32 = GROWS_AT + EYE_SLOTS * DECORS.len() as u32;

/// How far about its own, in its window's cells, a candidate's growth reads
/// the world (`grown`): the cells its pose votes on, its box, and the placed
/// decor that may claim it, none wider than the widest decor.
const READ_REACH: i32 = ALIGN_NEIGHBOR_RADIUS + WIDEST;

/// The leaf cells, `[x0, y0, x1, y1)`, of square `q` of the window at `cell`.
pub const fn square_box(cell: Zoom, q: [i32; 2]) -> [i32; 4] {
    let k = SQUARE_SHIFT + LEAF_CELL - cell;
    [q[0] << k, q[1] << k, (q[0] + 1) << k, (q[1] + 1) << k]
}

/// The leaf cells a candidate at leaf cell `c` reads where it grows at
/// `cell`: its window's cells within `READ_REACH` of its own.
pub const fn reads(cell: Zoom, c: [i32; 2]) -> [i32; 4] {
    let s = LEAF_CELL - cell;
    let (x, y) = (c[0] >> s, c[1] >> s);
    [
        (x - READ_REACH) << s,
        (y - READ_REACH) << s,
        (x + READ_REACH + 1) << s,
        (y + READ_REACH + 1) << s,
    ]
}

/// The leaf cells any candidate in square `q` of the window at `cell` reads
/// (`reads`).
pub const fn square_reads(cell: Zoom, q: [i32; 2]) -> [i32; 4] {
    let ([x0, y0, x1, y1], r) = (square_box(cell, q), READ_REACH << (LEAF_CELL - cell));
    [x0 - r, y0 - r, x1 + r, y1 + r]
}

/// Whether the box `b` of leaf cells meets the leaf cells `reads`
/// (`reads`, `square_reads`), each X on any lap, X round the world.
pub fn meets(reads: [i32; 4], b: [i32; 4]) -> bool {
    let wide = reads[2] - reads[0];
    // The box's first column from the region's, the short way round.
    let (from, width) = (wrap_index(LEAF_CELL, b[0] - reads[0]), b[2] - b[0]);
    // Met where it starts in the region or over it, or runs on round the
    // world into it.
    let across = width >= 1 << LEAF_CELL
        || (from < wide && from + width > 0)
        || from + width > 1 << LEAF_CELL;
    across && b[1] < reads[3] && b[3] > reads[1]
}

/// The zoom a record names where it is drawn at any distance: placed decor,
/// which no window grows. No window's cells are at it (`FIRST_CELL`).
pub const PLACED_AT: Zoom = 0;
const _: () = assert!(crate::window::FIRST_CELL > PLACED_AT);

/// A record's first word: its decor and variant, a byte each, its turn in
/// quarters, and the window zoom it was grown at, the one it is drawn at
/// where the rays ask it (`drawn_at`), `PLACED_AT` for placed decor; past
/// them, once sized, its region (`drawn::REGION_SHIFT`).
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

/// How the GPU draws what it grows: each record sized for the screen, a
/// model or a sprite, into its group's region, models at `2·group` and
/// sprites past it; the regions laid end to end in the decor's instances, in the
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
    /// indexed, a sprite's a strip of four corners an instance; its first
    /// instance 0, which a device without `indirect-first-instance` holds
    /// every indirect draw to.
    pub const ARGS_WORDS: u32 = 5;
    /// Each region's draw uniform, `UNIFORM_WORDS` apart (`OFFSET_BYTES`, every
    /// device's dynamic offset alignment), its index list's first slot at
    /// `UNIFORM_INDEX` within it: the lanes' base, to which the args pass adds
    /// the region's first slot once the regions are laid out.
    pub const UNIFORM_WORDS: u32 = OFFSET_BYTES / 4;
    /// The dynamic offset alignment every WebGPU device meets: its
    /// `minUniformBufferOffsetAlignment` is 256 bytes at most.
    const OFFSET_BYTES: u32 = 256;
    pub const UNIFORM_INDEX: u32 = 7;
    /// The drawing words, what a frame's decor is drawn by: the model's and
    /// the sprite's floors, each an angle about the eye in radians, the
    /// anchor the lanes' positions are about (unit cells, X wrapping), the
    /// left-out decor's corner and whether there is one, then each group's
    /// extent.
    pub const MODEL_ANGLE: u32 = 0;
    pub const SPRITE_ANGLE: u32 = 1;
    pub const ANCHOR: u32 = 4;
    pub const HIDE: u32 = 8;
    pub const EXTENTS: u32 = 12;
    pub const DRAWING_WORDS: u32 = EXTENTS + GROUPS;

    /// What a thing spanning `across` radians about the eye is drawn as: a
    /// model at `model` or more, else a sprite at `sprite` or more, else
    /// nothing, each an angle, so no screen decides it. A group's region is
    /// `2·group` past its tier where drawn.
    pub const MODEL: u32 = 0;
    pub const SPRITE: u32 = 1;
    pub const GONE: u32 = 2;

    /// The tier of what spans `across` radians, against the model's and the
    /// sprite's floors: the one rule decor's count and the CPU's clouds
    /// both size by.
    pub fn tier(across: f32, model: f32, sprite: f32) -> u32 {
        if across >= model {
            MODEL
        } else if across >= sprite {
            SPRITE
        } else {
            GONE
        }
    }
}

/// Whether `a` stands before `b` where both would claim a cell: the one born
/// coarser, so a zoom keeps what it had, then the lower seed.
pub fn before(a: &Candidate, b: &Candidate) -> bool {
    a.born < b.born || (a.born == b.born && a.seed < b.seed)
}

/// Whether the squares two candidates claim, `sa` and `sb` cells from their
/// cells up, share a cell.
pub fn meet(a: &Candidate, sa: i32, b: &Candidate, sb: i32) -> bool {
    a.cell[0] < b.cell[0] + sb
        && b.cell[0] < a.cell[0] + sa
        && a.cell[1] < b.cell[1] + sb
        && b.cell[1] < a.cell[1] + sa
}

/// The high word of `a × b`, in u32 alone.
fn mul_hi(a: u32, b: u32) -> u32 {
    let (a0, a1, b0, b1) = (a & 0xffff, a >> 16, b & 0xffff, b >> 16);
    let (low, cross0, cross1) = (a0 * b0, a1 * b0, a0 * b1);
    let carry = (low >> 16) + (cross0 & 0xffff) + (cross1 & 0xffff);
    a1 * b1 + (cross0 >> 16) + (cross1 >> 16) + (carry >> 16)
}

/// The variant a candidate draws, its seed's first draw: uniform over the
/// decor's.
pub fn variant_of(decor: &Decor, draws: &mut SplitMix32) -> u32 {
    mul_hi(draws.next_u32(), decor.variants)
}

/// The square a candidate claims, which holds its box however it turns.
pub fn side_of(decor: &Decor, c: &Candidate) -> i32 {
    let [x, y, _] = decor.sizes[variant_of(decor, &mut SplitMix32::new(c.seed)) as usize];
    x.max(y) as i32
}

/// Whether `d`, its square `sd` a side, keeps `c`, its `sc`, from standing:
/// it stands before it and claims a cell of its square. The one rule of the
/// contest (`stands`).
pub fn beats(d: &Candidate, sd: i32, c: &Candidate, sc: i32) -> bool {
    before(d, c) && meet(d, sd, c, sc)
}

/// The leaf cells a candidate of `decor` beating `c`, its square `side` a
/// side, stands in (`beats`): as far before it as the widest decor's
/// square reaches, to the end of its own.
pub fn rivals(decor: &Decor, c: &Candidate, side: i32) -> [[i32; 2]; 2] {
    let reach = decor.widest() as i32 - 1;
    [
        [c.cell[0] - reach, c.cell[1] - reach],
        [c.cell[0] + side, c.cell[1] + side],
    ]
}

/// A patch's candidates as bits, a word each 32 of them.
pub const MARKS: usize = COUNTS.div_ceil(32);

/// Whether candidate `i` is set in `m`.
pub fn marked(m: &[u32; MARKS], i: usize) -> bool {
    m[i / 32] >> (i % 32) & 1 != 0
}

/// Candidate `i` set in `m`.
pub fn mark(m: &mut [u32; MARKS], i: usize) {
    m[i / 32] |= 1 << (i % 32);
}

/// Which of the first `n` candidates of a patch, `cs`, set in `asked`,
/// are beaten at `zoom` (`beats`): each its own answer of `stands`, their
/// rivals drawn once for them all, over the cells every asked one's rivals
/// stand in (`rivals`), not once each.
pub fn beaten(
    decor: &Decor,
    zoom: Zoom,
    cs: &[Candidate; COUNTS],
    n: u32,
    asked: &[u32; MARKS],
) -> [u32; MARKS] {
    let (mut sides, mut lo, mut hi) = ([0i32; COUNTS], [i32::MAX; 2], [i32::MIN; 2]);
    let mut i = 0;
    while i < n as usize {
        if marked(asked, i) {
            sides[i] = side_of(decor, &cs[i]);
            let [l, h] = rivals(decor, &cs[i], sides[i]);
            lo = [lo[0].min(l[0]), lo[1].min(l[1])];
            hi = [hi[0].max(h[0]), hi[1].max(h[1])];
        }
        i += 1;
    }
    let mut beaten = [0u32; MARKS];
    if lo[0] > hi[0] {
        return beaten;
    }
    each_candidate(decor, zoom, [lo, hi], |d| {
        let sd = side_of(decor, &d);
        let mut i = 0;
        while i < n as usize {
            if marked(asked, i) && beats(&d, sd, &cs[i], sides[i]) {
                mark(&mut beaten, i);
            }
            i += 1;
        }
    });
    beaten
}

/// Whether `c` stands at `zoom`: no candidate born there or coarser beats
/// it, a patch's contest of one (`beaten`).
pub fn stands(decor: &Decor, zoom: Zoom, c: &Candidate) -> bool {
    let (mut cs, mut asked) = ([Candidate::NONE; COUNTS], [0u32; MARKS]);
    cs[0] = *c;
    mark(&mut asked, 0);
    !marked(&beaten(decor, zoom, &cs, 1, &asked), 0)
}

/// What stands in one leaf cell on the leaves: its candidate before the rest
/// there, where it stands (`stands`); any other there it beats.
pub fn scatter_at(decor: &Decor, cell: [i32; 2]) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    each_candidate(decor, LEAF_ZOOM, [cell, [cell[0] + 1, cell[1] + 1]], |c| {
        if best.is_none_or(|b| before(&c, &b)) {
            best = Some(c);
        }
    });
    best.filter(|b| stands(decor, LEAF_ZOOM, b))
}

/// What growing decor asks of the world, as either side holds it: the
/// query's support, a column's footing and the assets about it, what is
/// placed in a box, and the catalog's rows where it reads them. Decor
/// reads no layer's storage.
pub trait Grounds {
    /// A footing as this side keeps it: what its assets are asked off.
    type Footing;
    /// The catalog's rows (`decor::DECORS`), as this side holds them: a GPU
    /// reads them from a buffer, never a table it copies.
    fn rows(&self) -> &[Decor];
    /// The footing down column `g` at `cell`, the query's support
    /// (`support::footing`), as this side asks it; its top (`cast::top`),
    /// false where nothing fills there or the world lacks it.
    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (Self::Footing, (i32, f32), bool);
    /// What is in the cell `c` of `zoom` about the footing `f`, the
    /// world's one query's answer (`support::asset`): `void`, the ground's
    /// value, or `PLACED_ASSET` over what is placed.
    fn asset(&mut self, f: &Self::Footing, zoom: Zoom, c: [i32; 3]) -> u32;
    /// The next leaf placed in `p`'s box, its cell and its word, false past
    /// the last (`world::Placed::next`).
    fn placed(&self, p: &mut Placed) -> ([i32; 3], u32, bool);
}

/// Decor the ground grows, where it stands: its variant, its heading in
/// quarter turns, its box's min corner, in leaf cells, the cell over its
/// footing's, and how far its base lies under that corner, in Q3 steps: the
/// footing's top kept short of its cube (`cast::top`), so it stands on the
/// top as drawn, not its cube's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grown {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
    pub sink: u32,
}

impl Grown {
    pub const NONE: Grown = Grown {
        variant: 0,
        turn: 0,
        at: [0; 3],
        sink: 0,
    };
}

/// Where candidate `c` of `decor` stands at `cell`: on its column's footing
/// (`support::footing`), facing as the cells beside it at the footing's
/// level vote (`asks`); then, where `boxed`, whether it grows there:
/// every cell of its box void and every one under its base a look it
/// grows on, whichever layer holds it, each asked of the world's one query
/// at `cell`; air placed in it or under it no void; and no placed decor
/// claims its room (`claimed`). Every cell both ask is asked at one place
/// about one footing (`asks`). Where it stands, whether it does, and
/// whether it grows there, true where not `boxed`.
pub fn grown(
    q: &mut impl Grounds,
    decor: &Decor,
    cell: Zoom,
    c: &Candidate,
    boxed: bool,
) -> (Grown, bool, bool) {
    let s = LEAF_CELL - cell;
    let at = [c.cell[0] >> s, c.cell[1] >> s];
    let (f, (floor, rise), here) = q.footing(cell, at);
    if !here {
        return (Grown::NONE, false, false);
    }
    let g = Grown {
        variant: 0,
        turn: 0,
        at: [c.cell[0], c.cell[1], (floor + 1) * (1 << s)],
        sink: ((1.0 - rise) * (1u32 << (s + FRAC_W)) as f32 + 0.5) as u32,
    };
    let (g, grows) = asks(q, &f, decor, cell, c.seed, g, boxed);
    (g, true, grows)
}

/// Every cell growing decor asks of the world, at one place, as a GPU
/// inlines a call once a place it is called from: a candidate posed by its
/// `seed`, the cells about its footing its pose votes on, then, where
/// `boxed`, the box of `g` as that pose turned it (`grown`'s rule).
/// The pose is the candidate's variant and its heading in quarter turns,
/// its seed's draws in order: along transport, a plurality vote of the
/// cells a way runs on, by their offset from the candidate's, along each
/// axis — a quarter turn if horizontal wins, else none (a tie or isolated
/// cell has no real answer), and a half more on a draw, as two-way traffic
/// faces both ways — the four at a distance in turn, before, after, under,
/// over. What stands, and whether it grows there.
fn asks<Q: Grounds>(
    q: &mut Q,
    f: &Q::Footing,
    decor: &Decor,
    cell: Zoom,
    seed: u32,
    mut g: Grown,
    boxed: bool,
) -> (Grown, bool) {
    let s = LEAF_CELL - cell;
    let (mut flip, mut voting) = (0, 0);
    let mut draws = SplitMix32::new(seed);
    g.variant = variant_of(decor, &mut draws);
    match decor.orientation {
        Orientation::random => g.turn = draws.next_u32() >> (u32::BITS - TURN_BITS),
        Orientation::along_transport => {
            flip = draws.next_u32() >> 31;
            voting = 4 * ALIGN_NEIGHBOR_RADIUS;
        }
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
                decor.allows(value(v))
            } else {
                v == sentinel::void as u32
            };
            if !holds {
                return (g, false);
            }
        } else if Ground::transports(value(v)) {
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
        !boxed || !claimed(q, g.at, [w as i32, d as i32, h as i32]),
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
/// drawn at `cell`: the zoom the rays ask at its distance (`Detail::at`),
/// so each decor is drawn at one zoom, where the rays draw the ground
/// about it. The ground's decor grows no finer than a leaf cell's window
/// (`shown`'s zooms), so where the rays ask finer, for what is placed
/// finer, it is drawn at the leaf cell's.
pub fn drawn_at(detail: Detail, d2: f32, cell: Zoom) -> bool {
    detail.at(d2).min(LEAF_CELL) == cell
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
pub fn row_of(rows: &[Decor], value: u32) -> (u32, bool) {
    let mut k = 0;
    while k < rows.len() {
        if rows[k].value == value {
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
            "decor is grown and claimed in unit cells"
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
/// stands on as anything grown reaches, and as far across (pxl-spec-core
/// sun.rs `Sun::up`, `Sun::ring`).
pub const TALLEST: i32 = CLAIM_REACH[2] + 1;
pub const WIDEST: i32 = CLAIM_REACH[0] + 1;

/// Whether placed decor claims a cell of the box from `at` (unit cells),
/// `cells` a side: one whose corner, posed as placed (`placed_pose`), holds
/// a box meeting it, each placed leaf near enough asked of the world's
/// (`Grounds::placed`).
pub fn claimed(q: &impl Grounds, at: [i32; 3], cells: [i32; 3]) -> bool {
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
        let (c, word, more) = q.placed(&mut placed);
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
    /// The widest side any variant's box has, in cells: two corners nearer
    /// than this on both axes may claim one cell.
    pub const fn widest(&self) -> u32 {
        let mut widest = 1;
        let mut v = 0;
        while v < self.variants as usize {
            let [x, y, _] = self.sizes[v];
            widest = if x > widest { x } else { widest };
            widest = if y > widest { y } else { widest };
            v += 1;
        }
        widest
    }

    /// The most candidates a patch of it bears, its counts' longest: a
    /// patch's room among a slot's records (`slot_record`).
    pub const fn room(&self) -> u32 {
        let [top, finer] = self.counts;
        if top.len > finer.len {
            top.len
        } else {
            finer.len
        }
    }

    /// `variant`'s box's sides in cells, turned `turn` quarter turns: an odd
    /// one swaps x and y, the box keeping its min corner.
    pub const fn cells(&self, variant: u32, turn: u32) -> [u32; 3] {
        let [x, y, z] = self.sizes[variant as usize];
        if turn.is_multiple_of(2) {
            [x, y, z]
        } else {
            [y, x, z]
        }
    }

    /// Whether the ground grows it on a cell of `value`: a look it stands on.
    pub const fn allows(&self, value: u32) -> bool {
        value < 256 && self.on[(value >> 5) as usize] >> (value & 31) & 1 == 1
    }

    /// Its cell's zoom: a unit's, `scale_steps` coarser.
    pub const fn cell_zoom(&self) -> Zoom {
        (crate::UNIT_ZOOM as i32 - self.scale_steps) as Zoom
    }
}

#[cfg(test)]
#[path = "scatter.test.rs"]
mod test;
