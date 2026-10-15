// pxl-portable/scatter.rs

//! gpu-portable: decor the ground grows on its own, never stored. A Poisson
//! process on the leaf cell lattice, never a grid: a block at zoom m bears a
//! count of candidates born there, each at a uniform leaf cell, and a
//! candidate born at m stands at every zoom from m finer, so a cell at any
//! zoom holds the kind's density. Every draw is a u32 off one stream a block
//! (xpute-kit `SplitMix32`), seeded by the kind's seed and the block
//! (`node_seed`), so every reader of a cell, either side, draws the same.

use xpute_kit::math::rng::{derive_seed_u32, SplitMix32};

use crate::caves::node_seed;
use crate::decor::{Counts, Decor, Orientation, BLOCK};
use crate::lattice::GROUND;
use crate::{Zoom, LEAF_ZOOM};

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

/// Every candidate of `decor` born at `zoom` or coarser in `[min, max)`
/// (leaf cells), block by block, coarsest first.
pub fn each_candidate(decor: &Decor, zoom: Zoom, [min, max]: [[i32; 2]; 2], mut each: impl FnMut(Candidate)) {
    let top = GROUND.top();
    let mut born = zoom + 1;
    while born > top {
        born -= 1;
        // A block's side in leaf cells, as a shift.
        let shift = BLOCK_SHIFT + GROUND.finest_in(born).trailing_zeros();
        let counts = &decor.counts[usize::from(born != top)];
        let mut by = min[1] >> shift;
        while by <= (max[1] - 1) >> shift {
            let mut bx = min[0] >> shift;
            while bx <= (max[0] - 1) >> shift {
                let block = node_seed(decor.seed, (born, [bx, by]));
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
pub fn pose(decor: &Decor, c: &Candidate, way: impl Fn([i32; 2]) -> bool) -> (u32, u32) {
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

/// What stands in one leaf cell on the leaves: its candidate that stands
/// before the rest there, unless one before it claims a cell of its square.
pub fn scatter_at(decor: &Decor, cell: [i32; 2]) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    each_candidate(decor, LEAF_ZOOM, [cell, [cell[0] + 1, cell[1] + 1]], |c| {
        if best.is_none_or(|b| before(&c, &b)) {
            best = Some(c);
        }
    });
    let best = best?;
    let side = side_of(decor, &best);
    let widest = decor.widest() as i32;
    let around = [[cell[0] - (widest - 1), cell[1] - (widest - 1)], [cell[0] + widest, cell[1] + widest]];
    let mut beaten = false;
    each_candidate(decor, LEAF_ZOOM, around, |d| beaten |= before(&d, &best) && meet(&d, side_of(decor, &d), &best, side));
    (!beaten).then_some(best)
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

    /// Whether the ground grows it on a cell of `look`.
    pub const fn allows(&self, look: u8) -> bool {
        self.on[(look >> 5) as usize] >> (look & 31) & 1 == 1
    }

    /// Its cell's zoom: a unit's, `scale_steps` coarser.
    pub const fn cell_zoom(&self) -> Zoom {
        (crate::UNIT_ZOOM as i32 - self.scale_steps) as Zoom
    }
}
