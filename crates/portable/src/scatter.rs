// pxl-portable/scatter.rs

//! gpu-portable: decor the ground grows on its own, never stored. A Poisson
//! process on the leaf cell lattice, never a grid: a block at zoom m bears a
//! count of candidates born there, each at a uniform leaf cell, and a
//! candidate born at m stands at every zoom from m finer, so a cell at any
//! zoom holds the kind's density. Every draw is a u32 off one stream a block
//! (xpute-kit `SplitMix32`), seeded by the kind's seed and the block
//! (`node_seed`), so every reader of a cell, either side, draws the same.
//!
//! Where a candidate stands, which way it faces and whether its footing
//! holds are functions of the world as either side holds it (`stand`,
//! `grows`, over `Grounds`), and so is what placed decor claims (`claimed`).

use xpute_kit::math::rng::{derive_seed_u32, SplitMix32};

use crate::cast::Detail;
use crate::caves::node_seed;
use crate::cube::{sentinel, Ground};
use crate::decor::{Counts, Decor, Orientation, BLOCK, DECORS};
use crate::lattice::GROUND;
use crate::overwrite::{self, Lanes, PLACED};
use crate::window::{EYE_WINDOWS, SLOTS, SQUARE_SHIFT};
use crate::{wrap_index, Zoom, FRAC_W, LEAF_CELL, LEAF_ZOOM, UNIT_ZOOM};

const BLOCK_SHIFT: u32 = BLOCK.trailing_zeros();
const _: () = assert!(BLOCK.is_power_of_two(), "a block's side is a power of two, so a uniform cell in it is a draw's top bits");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The leaf cell it stands at, at every zoom.
    pub cell: [i32; 2],
    /// The coarsest zoom it stands at.
    pub born: Zoom,
    /// Its own draws: variant, heading.
    pub seed: u32,
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

/// The block of `decor`'s process at `born` with index `b`: its seed, and its
/// side in leaf cells as a shift.
fn block_seed(decor: &Decor, born: Zoom, b: [i32; 2]) -> (u32, u32) {
    (node_seed(decor.seed, (born, b)), BLOCK_SHIFT + GROUND.finest_in(born).trailing_zeros())
}

/// Every candidate of `decor` born at `zoom` or coarser in `[min, max)`
/// (leaf cells), block by block, coarsest first.
pub fn each_candidate(decor: &Decor, zoom: Zoom, [min, max]: [[i32; 2]; 2], mut each: impl FnMut(Candidate)) {
    let top = GROUND.top();
    let mut born = zoom + 1;
    while born > top {
        born -= 1;
        let (_, shift) = block_seed(decor, born, [0, 0]);
        let counts = &decor.counts[usize::from(born != top)];
        let mut by = min[1] >> shift;
        while by <= (max[1] - 1) >> shift {
            let mut bx = min[0] >> shift;
            while bx <= (max[0] - 1) >> shift {
                let (block, _) = block_seed(decor, born, [bx, by]);
                let mut draws = SplitMix32::new(block);
                let n = counts.of(draws.next_u32());
                let mut i = 0;
                while i < n {
                    let x = (bx << shift) + (draws.next_u32() >> (32 - shift)) as i32;
                    let y = (by << shift) + (draws.next_u32() >> (32 - shift)) as i32;
                    if min[0] <= x && x < max[0] && min[1] <= y && y < max[1] {
                        each(Candidate {
                            cell: [x, y],
                            born,
                            seed: derive_seed_u32(block, i),
                        });
                    }
                    i += 1;
                }
                bx += 1;
            }
            by += 1;
        }
    }
}

/// The `n`-th block meeting `[min, max)` (leaf cells) of those born at
/// `zoom` or coarser, the finest born first: its zoom and index; false past
/// the last.
pub fn block_of(zoom: Zoom, [min, max]: [[i32; 2]; 2], n: u32) -> (Zoom, [i32; 2], bool) {
    let top = GROUND.top();
    let (mut born, mut n) = (zoom + 1, n);
    while born > top {
        born -= 1;
        let shift = BLOCK_SHIFT + GROUND.finest_in(born).trailing_zeros();
        let first = [min[0] >> shift, min[1] >> shift];
        let across = [((max[0] - 1) >> shift) - first[0] + 1, ((max[1] - 1) >> shift) - first[1] + 1];
        let count = (across[0] * across[1]) as u32;
        if n < count {
            return (born, [first[0] + (n % across[0] as u32) as i32, first[1] + (n / across[0] as u32) as i32], true);
        }
        n -= count;
    }
    (top, [0, 0], false)
}

/// The most blocks `block_of` names over a window's square at any zoom: a
/// square at the leaves, its side `window::SQUARE` leaf cells, aligned to
/// it, so a block as wide or wider holds it whole.
pub const SQUARE_BLOCKS: u32 = {
    let side = crate::window::SQUARE_SHIFT;
    let (mut born, mut sum) = (LEAF_ZOOM + 1, 0);
    while born > GROUND.top() {
        born -= 1;
        let shift = BLOCK_SHIFT + GROUND.finest_in(born).trailing_zeros();
        let across = if side > shift { 1 << (side - shift) } else { 1 };
        sum += across * across;
    }
    sum
};

/// The most decor of every kind one square grows: a block bears no more
/// than its counts' thresholds, and no more blocks than `SQUARE_BLOCKS`
/// meet a square.
pub const SQUARE_GROWN: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        let [top, finer] = DECORS[k].counts;
        sum += if top.len > finer.len { top.len } else { finer.len };
        k += 1;
    }
    sum * SQUARE_BLOCKS
};

const _: () = assert!(size_of::<Decor>().is_multiple_of(4), "a row is whole words, as a GPU reads the table");

/// A record of decor the GPU draws: its first word (`record_word`), then
/// its box's min corner in leaf cells.
pub const GROWN_WORDS: u32 = 4;

/// The eye's windows' squares, a slot each: what the GPU grows decor in,
/// slot `s` the square the windows' table holds at `s`.
pub const EYE_SLOTS: u32 = EYE_WINDOWS * SLOTS;

/// The GPU's decor in one buffer of words: a head per eye slot, then each
/// slot's `SQUARE_GROWN` records, then placed decor's head and its records.
/// A head is its records' count, the square they were grown for, X and Y,
/// and 1 once they were: a slot whose square the windows no longer hold
/// there is skipped by it (`grown_for`). Placed decor's head is its count.
pub const HEAD_WORDS: u32 = 4;
const SLOTS_AT: u32 = EYE_SLOTS * HEAD_WORDS;
/// Records the slots hold, every one, ahead of placed decor's.
pub const SLOT_RECORDS: u32 = EYE_SLOTS * SQUARE_GROWN;
pub const PLACED_HEAD: u32 = SLOTS_AT + SLOT_RECORDS * GROWN_WORDS;

/// Slot `s`'s head.
pub const fn head_at(s: u32) -> u32 {
    s * HEAD_WORDS
}

/// Record `n`, the slots' first, slot by slot, then placed decor's: its
/// first word, its head, and its index under that head, which the head's
/// count bounds.
pub const fn record_at(n: u32) -> (u32, u32, u32) {
    if n < SLOT_RECORDS {
        (SLOTS_AT + n * GROWN_WORDS, head_at(n / SQUARE_GROWN), n % SQUARE_GROWN)
    } else {
        let i = n - SLOT_RECORDS;
        (PLACED_HEAD + HEAD_WORDS + i * GROWN_WORDS, PLACED_HEAD, i)
    }
}

/// Whether a slot's head says its records were grown for the square `q`.
pub const fn grown_for(head: [u32; 4], q: [i32; 2]) -> bool {
    head[3] == 1 && head[1] as i32 == q[0] && head[2] as i32 == q[1]
}

/// The edits a frame hands decor's growth at most (pxl-kernel
/// scene/decor/drawn.rs): each the columns an overwrite wrote, a box of
/// leaf cells `[x0, y0, x1, y1)`, X on any lap; past them the kernel unions
/// the rest into the last.
pub const EDITS: u32 = 8;
/// The boxes a frame's growth is asked over: the edits, and a preview's
/// cut and left-out leaf as they were and as they are; past them the
/// renderer joins the rest into the last (pxl-render gpu/grown.rs `hold`).
pub const BOXES: u32 = EDITS + 4;
/// What `decor_mark_cs` reads first: whether every slot grows, decor having
/// been off; the squares filled this frame; the boxes; then the boxes, four
/// words each.
pub const MARKS_AT: u32 = 4;
pub const MARKS_WORDS: u32 = MARKS_AT + 4 * BOXES;
/// What `decor_mark_cs` writes and `decor_cs` is dispatched by: its
/// workgroups across, up and the slot-kinds listed, a pad, then the list,
/// a slot and its kind (`<< 16`) each.
pub const GROWS_AT: u32 = 4;

/// The leaf cells a preview's entry at `zoom` and `at` covers, as a box.
pub const fn cut_box(zoom: Zoom, at: [i32; 3]) -> [i32; 4] {
    if zoom <= LEAF_CELL {
        let s = LEAF_CELL - zoom;
        [at[0] << s, at[1] << s, (at[0] + 1) << s, (at[1] + 1) << s]
    } else {
        let s = zoom - LEAF_CELL;
        [at[0] >> s, at[1] >> s, (at[0] >> s) + 1, (at[1] >> s) + 1]
    }
}

/// Whether the box `b` of leaf cells meets square `q` of the window at
/// `cell` or a square beside it: what decor in the square stands and grows
/// on reads that far (`stand`'s faces), X round the world.
pub fn meets(cell: Zoom, q: [i32; 2], b: [i32; 4]) -> bool {
    let k = SQUARE_SHIFT + LEAF_CELL - cell;
    let side = 1i32 << k;
    let (lo, wide) = ([(q[0] << k) - side, (q[1] << k) - side], 3 * side);
    // The box's first column from the region's, the short way round.
    let (from, width) = (wrap_index(LEAF_CELL, b[0] - lo[0]), b[2] - b[0]);
    // Met where it starts in the region or over it, or runs on round the
    // world into it.
    let across = width >= 1 << LEAF_CELL || (from < wide && from + width > 0) || from + width > 1 << LEAF_CELL;
    across && b[1] < lo[1] + wide && b[3] > lo[1]
}

/// Whether squares `a` and `b` of the window at `cell` are one or beside
/// each other, X round the world.
pub fn beside(cell: Zoom, a: [i32; 2], b: [i32; 2]) -> bool {
    let (dx, dy) = (wrap_index(cell - SQUARE_SHIFT, a[0] - b[0]), a[1] - b[1]);
    dx.abs() <= 1 && dy.abs() <= 1
}

/// The zoom a record names where it is drawn at any distance: placed decor,
/// which no window grows. No window's cells are at it (`FIRST_CELL`).
pub const PLACED_AT: Zoom = 0;
const _: () = assert!(crate::window::FIRST_CELL > PLACED_AT);

/// A record's first word: its kind and variant, a byte each, its turn in
/// quarters, and the window zoom it was grown at, the one it is drawn at
/// where the rays ask it (`drawn_at`), `PLACED_AT` for placed decor; past
/// them, once sized, its region (`drawn::REGION_SHIFT`).
pub const fn record_word(kind: u32, variant: u32, turn: u32, cell: Zoom) -> u32 {
    kind | variant << 8 | turn << 16 | cell << 18
}

/// A record's kind, variant, turn and zoom (`record_word`).
pub const fn record_of(w: u32) -> (u32, u32, u32, Zoom) {
    (w & 0xff, (w >> 8) & 0xff, (w >> 16) & 3, (w >> 18) & ((1 << crate::ZOOM_BITS) - 1))
}

/// Every variant of every kind, numbered kind by kind in the catalog's
/// order: a decor's draw group (`first_group`).
pub const GROUPS: u32 = {
    let (mut sum, mut k) = (0, 0);
    while k < DECORS.len() {
        sum += DECORS[k].variants;
        k += 1;
    }
    sum
};

/// The draw group of `rows`' kind `kind`'s first variant.
pub fn first_group(rows: &[Decor], kind: u32) -> u32 {
    let (mut first, mut k) = (0, 0);
    while k < kind {
        first += rows[k as usize].variants;
        k += 1;
    }
    first
}

/// How the GPU draws what it grows: each record sized for the screen, a
/// model or a sprite, into its group's region, models at `2·group` and
/// sprites past it; the regions laid end to end in one decor block, in the
/// layout the decor and impostor shaders read (lanes x, y, z, spin, scale,
/// `DRAWN_LANES` a slot, then the index list), each region's draw a set of
/// indirect args.
pub mod drawn {
    use super::GROUPS;

    pub const REGIONS: u32 = 2 * GROUPS;
    /// A record drawn as nothing.
    pub const NONE: u32 = 0xff;
    /// What a record's first word holds past its kind, variant and turn:
    /// its region, once sized.
    pub const REGION_SHIFT: u32 = 24;
    pub const LANES: u32 = 5;
    /// Each region's indirect args, `ARGS_WORDS` a region: a model's
    /// indexed, a sprite's a strip of four corners an instance; its first
    /// instance 0, which a device without `indirect-first-instance` holds
    /// every indirect draw to.
    pub const ARGS_WORDS: u32 = 5;
    /// Each region's draw uniform, `UNIFORM_WORDS` apart (every device's
    /// dynamic offset alignment), its index list's first slot at
    /// `UNIFORM_INDEX` within it: the lanes' base, to which the args pass
    /// adds the region's first slot once the regions are laid out.
    pub const UNIFORM_WORDS: u32 = 64;
    pub const UNIFORM_INDEX: u32 = 7;
    /// The frame's words: a pixel's units at one unit's distance, the
    /// model's and the sprite's floors in pixels, the anchor the lanes'
    /// positions are about (unit cells, X wrapping), the left-out decor's
    /// corner and whether there is one, then each group's extent.
    pub const UPP: u32 = 0;
    pub const MODEL_PX: u32 = 1;
    pub const SPRITE_PX: u32 = 2;
    pub const ANCHOR: u32 = 4;
    pub const HIDE: u32 = 8;
    pub const EXTENTS: u32 = 12;
    pub const PARAM_WORDS: u32 = EXTENTS + GROUPS;
}

/// Candidate `i` of the `n`-th block meeting `region` (`block_of`); false
/// unless the block bears one that many and it lies in `region`.
pub fn candidate_of(decor: &Decor, zoom: Zoom, region: [[i32; 2]; 2], n: u32, i: u32) -> (Candidate, bool) {
    const NONE: Candidate = Candidate { cell: [0; 2], born: 0, seed: 0 };
    let (born, b, here) = block_of(zoom, region, n);
    if !here {
        return (NONE, false);
    }
    let (block, shift) = block_seed(decor, born, b);
    let mut draws = SplitMix32::new(block);
    if i >= decor.counts[usize::from(born != GROUND.top())].of(draws.next_u32()) {
        return (NONE, false);
    }
    // Two draws a candidate before it.
    let mut k = 0;
    while k < 2 * i {
        draws.next_u32();
        k += 1;
    }
    let x = (b[0] << shift) + (draws.next_u32() >> (32 - shift)) as i32;
    let y = (b[1] << shift) + (draws.next_u32() >> (32 - shift)) as i32;
    let [min, max] = region;
    let c = Candidate {
        cell: [x, y],
        born,
        seed: derive_seed_u32(block, i),
    };
    (c, min[0] <= x && x < max[0] && min[1] <= y && y < max[1])
}

/// Whether `a` stands before `b` where both would claim a cell: the one born
/// coarser, so a zoom keeps what it had, then the lower seed.
pub fn before(a: &Candidate, b: &Candidate) -> bool {
    a.born < b.born || (a.born == b.born && a.seed < b.seed)
}

/// Whether the squares two candidates claim, `sa` and `sb` cells from their
/// cells up, share a cell.
pub fn meet(a: &Candidate, sa: i32, b: &Candidate, sb: i32) -> bool {
    a.cell[0] < b.cell[0] + sb && b.cell[0] < a.cell[0] + sa && a.cell[1] < b.cell[1] + sb && b.cell[1] < a.cell[1] + sa
}

/// The high word of `a × b`, in u32 alone.
fn mul_hi(a: u32, b: u32) -> u32 {
    let (a0, a1, b0, b1) = (a & 0xffff, a >> 16, b & 0xffff, b >> 16);
    let (low, cross0, cross1) = (a0 * b0, a1 * b0, a0 * b1);
    let carry = (low >> 16) + (cross0 & 0xffff) + (cross1 & 0xffff);
    a1 * b1 + (cross0 >> 16) + (cross1 >> 16) + (carry >> 16)
}

/// The variant a candidate draws, its seed's first draw: uniform over the
/// kind's.
pub fn variant_of(decor: &Decor, draws: &mut SplitMix32) -> u32 {
    mul_hi(draws.next_u32(), decor.variants)
}

/// The square a candidate claims, which holds its box however it turns.
pub fn side_of(decor: &Decor, c: &Candidate) -> i32 {
    let [x, y, _] = decor.sizes[variant_of(decor, &mut SplitMix32::new(c.seed)) as usize];
    x.max(y) as i32
}

/// How far about a cell its turn is voted: a fixed radius (O(1) reads, no
/// flood fill), enough to outvote a one-cell stairstep jog in a straight run;
/// it does not resolve curves.
pub const ALIGN_NEIGHBOR_RADIUS: i32 = 3;

/// A candidate's variant and its heading in quarter turns, its seed's draws
/// in order. Along transport, a plurality vote of the cells `way` says are
/// one, by their offset from the candidate's, along each axis: a quarter
/// turn if horizontal wins, else none (a tie or isolated cell has no real
/// answer), and a half more on a draw, as two-way traffic faces both ways.
pub fn pose(decor: &Decor, c: &Candidate, mut way: impl FnMut([i32; 2]) -> bool) -> (u32, u32) {
    let mut draws = SplitMix32::new(c.seed);
    let variant = variant_of(decor, &mut draws);
    let turn = match decor.orientation {
        Orientation::random => draws.next_u32() >> 30,
        Orientation::along_transport => {
            let flip = draws.next_u32() >> 31;
            // Each of the cells about it asked at one place, the four at a
            // distance in turn — before, after, under, over — as a GPU
            // inlines a call once a place it is called from.
            let (mut horizontal, mut vertical) = (0, 0);
            let mut k = 0;
            while k < 4 * ALIGN_NEIGHBOR_RADIUS {
                let (d, side) = (k / 4 + 1, k % 4);
                let along = if side % 2 == 0 { -d } else { d };
                let vote = u32::from(way(if side < 2 { [along, 0] } else { [0, along] }));
                if side < 2 {
                    horizontal += vote;
                } else {
                    vertical += vote;
                }
                k += 1;
            }
            u32::from(horizontal > vertical) + 2 * flip
        }
    };
    (variant, turn)
}

/// Whether `c` stands at `zoom`: no candidate born there or coarser, before
/// it, claims a cell of its square.
pub fn stands(decor: &Decor, zoom: Zoom, c: &Candidate) -> bool {
    let side = side_of(decor, c);
    let reach = decor.widest() as i32 - 1;
    let around = [[c.cell[0] - reach, c.cell[1] - reach], [c.cell[0] + side, c.cell[1] + side]];
    let mut beaten = false;
    each_candidate(decor, zoom, around, |d| beaten |= before(&d, c) && meet(&d, side_of(decor, &d), c, side));
    !beaten
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

/// A block's word where what is placed answered it, over its value, as the
/// world's one query reads on the GPU (gpu-query shader `World`) and
/// `Grounds::block` hands it.
pub const PLACED_BLOCK: u32 = 1 << 16;

/// What growing decor asks of the world, as either side holds it: the
/// ground's cells, the world's one query by a descent, what is placed
/// (`Lanes`), and the catalog's rows where it reads them.
pub trait Grounds: Lanes {
    /// What a descent hands a node: what its ancestors found.
    type At: Copy;
    /// The catalog's rows (`decor::DECORS`), as this side holds them: a GPU
    /// reads them from a buffer, never a table it copies.
    fn rows(&self) -> &[Decor];
    /// The footing down column `g` at `cell` (`footing`), as this side
    /// casts it: the value of the block it is, as `block` answers one, and
    /// the block over it; false where nothing fills there or the world
    /// lacks it.
    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (u32, i32, bool);
    /// The node `c` of `zoom` with what its line from the root found: where
    /// a descent starts.
    fn start(&mut self, zoom: Zoom, c: [i32; 3]) -> Self::At;
    /// What is in the block `c` of `zoom` under the node of `from` that `at`
    /// was handed for, a child a zoom down from it, the world's one query's
    /// answer: `void`, the ground's value, or `PLACED_BLOCK` over what is
    /// placed.
    fn block(&mut self, at: &Self::At, from: Zoom, zoom: Zoom, c: [i32; 3]) -> u32;
}

/// Zooms over the blocks `[lo, hi]` of `zoom` to the node holding them
/// all: the bits the corners differ in, on the axis they differ most; more
/// than `zoom` where no node does, the world having a root a node of zoom 0.
fn holding(lo: [i32; 3], hi: [i32; 3]) -> u32 {
    let d = (lo[0] ^ hi[0]) | (lo[1] ^ hi[1]) | (lo[2] ^ hi[2]);
    32 - (d as u32).leading_zeros()
}

/// Decor the ground grows, where it stands: its variant, its heading in
/// quarter turns, and its box's min corner, in leaf cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grown {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
}

/// The footing down column `g` at `cell`: the first block that fills on
/// the world's one ray straight down the column's middle, at `cell`'s
/// blocks alone, from the top of the world (`HEIGHT_BITS`) — where a walker
/// stands (pxl-kernel query/support.rs `highest`), whichever layer holds
/// it. Its block as `q` answers it, and the block over it, at `cell`;
/// false where the ray meets none.
pub fn footing<Q: crate::cast::Query>(q: &mut Q, cell: Zoom, g: [i32; 2]) -> (Q::Block, i32, bool) {
    use crate::cast::{first, Detail, Ends, Origin, Ray, Until};
    let s = crate::FINEST_ZOOM - cell;
    let mid = |v: i32| v.wrapping_shl(s).wrapping_add((1 << s) >> 1);
    let top = ((1i32 << (crate::HEIGHT_BITS - 1)) << FRAC_W) - 1;
    let o = Origin::of([mid(g[0]), mid(g[1]), top]);
    let ray = Ray::new([0.0, 0.0, -1.0], (1u32 << crate::HEIGHT_BITS) as f32);
    let met = first(q, &o, &ray, Detail { tau: 1.0, range: [cell, cell] }, Until::Fills);
    (met.stop.block, met.stop.place.node(&o)[2] + 1, met.ends == Ends::Stop)
}

/// The value a block answer holds, without whether what is placed holds it
/// (`PLACED_BLOCK`): a look reads the same whichever layer holds it.
fn value(v: u32) -> u32 {
    v & (PLACED_BLOCK - 1)
}

/// Where candidate `c` of `decor` stands at `cell`: on its column's footing
/// (`footing`), facing as the blocks beside it at the footing's level vote
/// (`pose`), each asked of the world's one query, whichever layer holds
/// them; false where its column has none. Whether it grows there is
/// `grows`'.
pub fn stand(q: &mut impl Grounds, decor: &Decor, cell: Zoom, c: &Candidate) -> (Grown, bool) {
    let s = LEAF_CELL - cell;
    let at = [c.cell[0] >> s, c.cell[1] >> s];
    let (_, top, here) = q.footing(cell, at);
    if !here {
        return (Grown { variant: 0, turn: 0, at: [0; 3] }, false);
    }
    let (variant, turn) = pose(decor, c, |[dx, dy]| {
        let b = [at[0] + dx, at[1] + dy, top - 1];
        let from = q.start(cell, b);
        Ground::transports(value(q.block(&from, cell, cell, b)))
    });
    let g = Grown {
        variant,
        turn,
        at: [c.cell[0], c.cell[1], top * (1 << s)],
    };
    (g, true)
}

/// Whether `decor` standing as `g` grows at `cell`: every block of its box
/// void and every one under its base a look it grows on, whichever layer
/// holds it, each asked of the world's one query at `cell` down from the node
/// holding them all, descended to once; air placed in it or under it no
/// void; and no placed decor claims its room (`claimed`).
pub fn grows(q: &mut impl Grounds, decor: &Decor, cell: Zoom, g: &Grown) -> bool {
    let s = LEAF_CELL - cell;
    let [w, d, h] = decor.cells(g.variant, g.turn);
    let (w, d, h) = (w as i32, d as i32, h as i32);
    let [x0, y0, z] = g.at;
    let lo = [x0 >> s, y0 >> s, (z - 1) >> s];
    let hi = [(x0 + w - 1) >> s, (y0 + d - 1) >> s, (z + h - 1) >> s];
    let k = holding(lo, hi);
    let held = k <= cell;
    let start = q.start(if held { cell - k } else { cell }, if held { [lo[0] >> k, lo[1] >> k, lo[2] >> k] } else { lo });
    // The box's columns, each from the block under its base up: one ask a
    // block, down from the node holding the box, or from its own root where
    // none does (`holding`).
    let mut by = lo[1];
    while by <= hi[1] {
        let mut bx = lo[0];
        while bx <= hi[0] {
            let mut bz = lo[2];
            while bz <= hi[2] {
                let c = [bx, by, bz];
                let (at, from) = if held { (start, cell - k) } else { (q.start(cell, c), cell) };
                let v = q.block(&at, from, cell, c);
                let holds = if bz == lo[2] { decor.allows(value(v)) } else { v == sentinel::void as u32 };
                if !holds {
                    return false;
                }
                bz += 1;
            }
            bx += 1;
        }
        by += 1;
    }
    !claimed(q, g.at, [w, d, h])
}

/// How far decor standing at `at` (leaf cells) lies from `origin` (Q3
/// steps), in units, squared: from its cell's middle at its base, X round
/// the world as the steps wrap.
pub fn distance2(origin: [i32; 3], at: [i32; 3]) -> f32 {
    let half = 1 << (FRAC_W - 1);
    let step = |v: i32, o: i32, mid: i32| (v.wrapping_shl(FRAC_W).wrapping_add(mid).wrapping_sub(o)) as f32 / (1 << FRAC_W) as f32;
    let [dx, dy, dz] = [step(at[0], origin[0], half), step(at[1], origin[1], half), step(at[2], origin[2], 0)];
    dx * dx + dy * dy + dz * dz
}

/// How far decor standing at `at` lies from `origin`, in units
/// (`distance2`'s root).
pub fn distance(origin: [i32; 3], at: [i32; 3]) -> f32 {
    libm::sqrtf(distance2(origin, at))
}

/// Whether decor standing at `at` is drawn at `cell` for an eye at
/// `origin`: the zoom the rays ask at its distance (`Detail::at`), so each
/// decor is drawn at one zoom, where the rays draw the ground about it.
pub fn drawn_at(detail: Detail, origin: [i32; 3], at: [i32; 3], cell: Zoom) -> bool {
    detail.at(distance2(origin, at)) == cell
}

/// Placed decor's tag (`node_seed`).
const PLACED_TAG: u32 = 0x91ac_ed5e;

/// A placed decor's variant and heading, seeded only by the unit cell of its
/// corner, its one stable identity, so a preview predicts exactly what a
/// placement gets.
pub fn placed_pose(decor: &Decor, corner: [i32; 3]) -> (u32, u32) {
    let mut draws = SplitMix32::new(node_seed(PLACED_TAG, (UNIT_ZOOM, corner)));
    let variant = variant_of(decor, &mut draws);
    (variant, draws.next_u32() >> 30)
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
/// may lie and its box still meet it: any kind's widest side and tallest
/// box, less one.
const CLAIM_REACH: [i32; 3] = {
    let (mut wide, mut tall, mut k) = (1, 1, 0);
    while k < DECORS.len() {
        let d = &DECORS[k];
        assert!(d.scale_steps == 0, "decor is grown and claimed in unit cells");
        wide = if d.widest() > wide { d.widest() } else { wide };
        let mut v = 0;
        while v < d.variants as usize {
            tall = if d.sizes[v][2] > tall { d.sizes[v][2] } else { tall };
            v += 1;
        }
        k += 1;
    }
    [wide as i32 - 1, wide as i32 - 1, tall as i32 - 1]
};

/// The nodes placed decor is first asked at, so a box with nothing placed
/// near it is told so in a few looks: 8 unit cells a side.
const CLAIM_ZOOM: Zoom = UNIT_ZOOM - 3;

/// Whether placed decor claims a cell of the box from `at` (unit cells),
/// `cells` a side: one whose corner, posed as placed (`placed_pose`), holds
/// a box meeting it.
pub fn claimed(lanes: &impl Grounds, at: [i32; 3], cells: [i32; 3]) -> bool {
    let lo = [at[0] - CLAIM_REACH[0], at[1] - CLAIM_REACH[1], at[2] - CLAIM_REACH[2]];
    let hi = [at[0] + cells[0] - 1, at[1] + cells[1] - 1, at[2] + cells[2] - 1];
    let s = UNIT_ZOOM - CLAIM_ZOOM;
    let mut n = [lo[0] >> s, lo[1] >> s, lo[2] >> s];
    while n[2] <= hi[2] >> s {
        n[1] = lo[1] >> s;
        while n[1] <= hi[1] >> s {
            n[0] = lo[0] >> s;
            while n[0] <= hi[0] >> s {
                let node = overwrite::cursor(lanes, PLACED, CLAIM_ZOOM, n);
                if node.holds() && claimed_in(lanes, n, node, lo, hi, at, cells) {
                    return true;
                }
                n[0] += 1;
            }
            n[1] += 1;
        }
        n[2] += 1;
    }
    false
}

/// `claimed`, over the corners in `[lo, hi]` within the node `n` at
/// `CLAIM_ZOOM`, holding `node`, each a placed decor's leaf or not, stepped
/// down from it.
fn claimed_in(lanes: &impl Grounds, n: [i32; 3], node: overwrite::Held, lo: [i32; 3], hi: [i32; 3], at: [i32; 3], cells: [i32; 3]) -> bool {
    let s = UNIT_ZOOM - CLAIM_ZOOM;
    let first = [lo[0].max(n[0] << s), lo[1].max(n[1] << s), lo[2].max(n[2] << s)];
    let last = [hi[0].min((n[0] << s) + (1 << s) - 1), hi[1].min((n[1] << s) + (1 << s) - 1), hi[2].min((n[2] << s) + (1 << s) - 1)];
    let mut u = first;
    while u[2] <= last[2] {
        u[1] = first[1];
        while u[1] <= last[1] {
            u[0] = first[0];
            while u[0] <= last[0] {
                let mut held = node;
                let mut z = CLAIM_ZOOM;
                while z < UNIT_ZOOM {
                    z += 1;
                    let t = UNIT_ZOOM - z;
                    held = overwrite::child(lanes, held, z, [u[0] >> t, u[1] >> t, u[2] >> t]);
                }
                if held.holds() && !held.folded() && held.zoom() == UNIT_ZOOM {
                    let (k, decor) = row_of(lanes.rows(), lanes.data(held.slot()) & 0xffff);
                    if decor {
                        let d = &lanes.rows()[k as usize];
                        let (variant, turn) = placed_pose(d, u);
                        let [w, e, h] = d.cells(variant, turn);
                        let (w, e, h) = (w as i32, e as i32, h as i32);
                        if u[0] < at[0] + cells[0] && at[0] < u[0] + w && u[1] < at[1] + cells[1] && at[1] < u[1] + e && u[2] < at[2] + cells[2] && at[2] < u[2] + h {
                            return true;
                        }
                    }
                }
                u[0] += 1;
            }
            u[1] += 1;
        }
        u[2] += 1;
    }
    false
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

    /// Whether the ground grows it on a block of `value`: a look it stands on.
    pub const fn allows(&self, value: u32) -> bool {
        value < 256 && self.on[(value >> 5) as usize] >> (value & 31) & 1 == 1
    }

    /// Its cell's zoom: a unit's, `scale_steps` coarser.
    pub const fn cell_zoom(&self) -> Zoom {
        (crate::UNIT_ZOOM as i32 - self.scale_steps) as Zoom
    }
}
