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

use crate::caves::node_seed;
use crate::cube::{sentinel, Ground};
use crate::decor::{Counts, Decor, Orientation, BLOCK, DECORS};
use crate::lattice::GROUND;
use crate::placed::{self, Lanes, PLACED};
use crate::{Zoom, LEAF_CELL, LEAF_ZOOM, UNIT_ZOOM};

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

/// Candidate `i` of the `n`-th block meeting `region` (`block_of`), where
/// the block bears one that many and it lies in `region`.
pub fn candidate_of(decor: &Decor, zoom: Zoom, region: [[i32; 2]; 2], n: u32, i: u32) -> Option<Candidate> {
    let (born, b, here) = block_of(zoom, region, n);
    if !here {
        return None;
    }
    let (block, shift) = block_seed(decor, born, b);
    let mut draws = SplitMix32::new(block);
    if i >= decor.counts[usize::from(born != GROUND.top())].of(draws.next_u32()) {
        return None;
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
    (min[0] <= x && x < max[0] && min[1] <= y && y < max[1]).then_some(Candidate {
        cell: [x, y],
        born,
        seed: derive_seed_u32(block, i),
    })
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
            let (mut horizontal, mut vertical) = (0, 0);
            let mut d = 1;
            while d <= ALIGN_NEIGHBOR_RADIUS {
                horizontal += u32::from(way([-d, 0])) + u32::from(way([d, 0]));
                vertical += u32::from(way([0, -d])) + u32::from(way([0, d]));
                d += 1;
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
/// ground's cells, the world's one query, and what is placed (`Lanes`).
pub trait Grounds: Lanes {
    /// The ground's cell `g` at `cell`, as a grid reads it: its face's look
    /// and its top, in blocks of `cell`; false where it has no ground.
    fn face(&mut self, cell: Zoom, g: [i32; 2]) -> (u32, i32, bool);
    /// What is in the block `c` of `zoom`, the world's one query's answer:
    /// `void`, the ground's value, or `PLACED_BLOCK` over what is placed.
    fn block(&mut self, zoom: Zoom, c: [i32; 3]) -> u32;
}

/// Decor the ground grows, where it stands: its variant, its heading in
/// quarter turns, and its box's min corner, in leaf cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grown {
    pub variant: u32,
    pub turn: u32,
    pub at: [i32; 3],
}

/// Where candidate `c` of `decor` stands on the ground's cells at `cell`, as
/// those cells draw it: on its cell's top, facing as its cells' looks vote
/// (`pose`); none where its cell has no ground. Whether its footing holds
/// there is `grows`'.
pub fn stand(q: &mut impl Grounds, decor: &Decor, cell: Zoom, c: &Candidate) -> Option<Grown> {
    let s = LEAF_CELL - cell;
    let at = [c.cell[0] >> s, c.cell[1] >> s];
    let (_, top, here) = q.face(cell, at);
    if !here {
        return None;
    }
    let (variant, turn) = pose(decor, c, |[dx, dy]| {
        let (look, _, here) = q.face(cell, [at[0] + dx, at[1] + dy]);
        here && look < 256 && Ground::of(look as u8).is_some_and(Ground::transport)
    });
    Some(Grown {
        variant,
        turn,
        at: [c.cell[0], c.cell[1], top * (1 << s)],
    })
}

/// Whether `decor` standing as `g` grows on the ground's cells at `cell`:
/// every block of its box void and every one under its base ground it grows
/// on, each asked of the world's one query at `cell`, air placed in it or
/// under it no void; and no placed decor claims its room (`claimed`).
pub fn grows(q: &mut impl Grounds, decor: &Decor, cell: Zoom, g: &Grown) -> bool {
    let s = LEAF_CELL - cell;
    let [w, d, h] = decor.cells(g.variant, g.turn).map(|n| n as i32);
    let [x0, y0, z] = g.at;
    let mut by = y0 >> s;
    while by <= (y0 + d - 1) >> s {
        let mut bx = x0 >> s;
        while bx <= (x0 + w - 1) >> s {
            let under = q.block(cell, [bx, by, (z - 1) >> s]);
            if under & PLACED_BLOCK != 0 || !decor.allows(under) {
                return false;
            }
            let mut bz = z >> s;
            while bz <= (z + h - 1) >> s {
                if q.block(cell, [bx, by, bz]) != sentinel::void as u32 {
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

/// The catalog's row a stored value names; false for one not decor.
pub fn row_of(value: u32) -> (u32, bool) {
    let mut k = 0;
    while k < DECORS.len() {
        if DECORS[k].value == value {
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
pub fn claimed(lanes: &impl Lanes, at: [i32; 3], cells: [i32; 3]) -> bool {
    let lo = [at[0] - CLAIM_REACH[0], at[1] - CLAIM_REACH[1], at[2] - CLAIM_REACH[2]];
    let hi = [at[0] + cells[0] - 1, at[1] + cells[1] - 1, at[2] + cells[2] - 1];
    let s = UNIT_ZOOM - CLAIM_ZOOM;
    let mut n = [lo[0] >> s, lo[1] >> s, lo[2] >> s];
    while n[2] <= hi[2] >> s {
        n[1] = lo[1] >> s;
        while n[1] <= hi[1] >> s {
            n[0] = lo[0] >> s;
            while n[0] <= hi[0] >> s {
                if placed::cursor(lanes, PLACED, CLAIM_ZOOM, n).holds() && claimed_in(lanes, n, lo, hi, at, cells) {
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
/// `CLAIM_ZOOM`, each a placed decor's leaf or not.
fn claimed_in(lanes: &impl Lanes, n: [i32; 3], lo: [i32; 3], hi: [i32; 3], at: [i32; 3], cells: [i32; 3]) -> bool {
    let s = UNIT_ZOOM - CLAIM_ZOOM;
    let first = [lo[0].max(n[0] << s), lo[1].max(n[1] << s), lo[2].max(n[2] << s)];
    let last = [hi[0].min((n[0] << s) + (1 << s) - 1), hi[1].min((n[1] << s) + (1 << s) - 1), hi[2].min((n[2] << s) + (1 << s) - 1)];
    let mut u = first;
    while u[2] <= last[2] {
        u[1] = first[1];
        while u[1] <= last[1] {
            u[0] = first[0];
            while u[0] <= last[0] {
                let held = placed::cursor(lanes, PLACED, UNIT_ZOOM, u);
                if held.holds() && !held.folded() && held.zoom() == UNIT_ZOOM {
                    let (k, decor) = row_of(lanes.data(held.slot()) & 0xffff);
                    if decor {
                        let d = &DECORS[k as usize];
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
