// pxl-spec-core/scatter.rs

//! gpu-portable: the decor the ground holds, never stored, as the caves are:
//! a hash of a leaf cell, a unit, drawn against a decor's chance says the
//! cell holds one (`candidate_at`), its variant and heading its own draws
//! after; one whose box meets a neighbor's that drew lower gives way to it
//! (`retained`). At any zoom the same cells are read: what is as large as a
//! cell there stands as itself (`stands_at`), what is smaller is folded into
//! the cell as a cube is, its room to the cell's kept eighths (`folded`), and
//! what folds to nothing is none. Every draw is a u32 off one stream a cell
//! (xpute-kit `SplitMix32`), seeded by the decor's seed and the cell
//! (`node_seed`), so every reader of a cell, either side, draws the same.

use xpute_kit::math::rng::SplitMix32;

use crate::answer;
use crate::cave::node_seed;
use crate::decor::{Decor, DECORS};
use crate::{Zoom, LEAF_CELL, UNIT_ZOOM};

/// A cell's candidate: the leaf cell it stands at, at every zoom; its first
/// draw, against the decor's chance and, lower first, the order neighbors
/// give way in (`before`); and the seed of its own draws after (variant,
/// heading).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub cell: [i32; 2],
    pub draw: u32,
    pub seed: u32,
}

impl Candidate {
    pub const NONE: Candidate = Candidate {
        cell: [0; 2],
        draw: u32::MAX,
        seed: 0,
    };
}

/// Leaf cell `cell`'s candidate of `decor`, and whether the cell holds one:
/// its first draw under the decor's chance (`Decor::chance`).
pub fn candidate_at(decor: &Decor, cell: [i32; 2]) -> (Candidate, bool) {
    drawn(decor.seed, decor.chance, cell)
}

/// `candidate_at` of a decor's seed and chance, by value.
fn drawn(seed: u32, chance: u32, cell: [i32; 2]) -> (Candidate, bool) {
    let mut draws = SplitMix32::new(node_seed(seed, (LEAF_CELL, cell)));
    let draw = draws.next_u32();
    (
        Candidate {
            cell,
            draw,
            seed: draws.next_u32(),
        },
        draw < chance,
    )
}

/// Whether `a` comes before `b` where their boxes meet, the one giving way to
/// the other (`retained`): the lower first draw, then the lower cell.
pub fn before(a: &Candidate, b: &Candidate) -> bool {
    a.draw < b.draw
        || (a.draw == b.draw
            && (a.cell[1] < b.cell[1] || (a.cell[1] == b.cell[1] && a.cell[0] < b.cell[0])))
}

/// Whether the bricks two candidates claim, `sa` and `sb` cells from their
/// cells up, share a cell.
pub fn meet(a: &Candidate, sa: i32, b: &Candidate, sb: i32) -> bool {
    a.cell[0] < b.cell[0] + sb
        && b.cell[0] < a.cell[0] + sa
        && a.cell[1] < b.cell[1] + sb
        && b.cell[1] < a.cell[1] + sa
}

/// The high word of `a × b`, in u32 alone.
pub const fn mul_hi(a: u32, b: u32) -> u32 {
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

/// The variant candidate `c` draws (`variant_of`), the first of its seed's.
pub fn variant(decor: &Decor, c: &Candidate) -> u32 {
    variant_of(decor, &mut SplitMix32::new(c.seed))
}

/// The brick a candidate claims, which holds its box however it turns.
pub fn side_of(decor: &Decor, c: &Candidate) -> i32 {
    let [x, y, _] = decor.sizes[variant(decor, c) as usize];
    x.max(y) as i32
}

/// Whether `c` stands clear of `decor`'s other candidates: none before it
/// (`before`) claims a brick meeting its own (`side_of`, `meet`), whether or
/// not that one gives way itself, as Matérn's second thinning keeps one. The
/// cells asked are those a brick meeting `c`'s may start at, as wide as the
/// decor's widest.
pub fn retained(decor: &Decor, c: &Candidate) -> bool {
    let (side, reach) = (side_of(decor, c), decor.widest() as i32 - 1);
    let mut y = c.cell[1] - reach;
    while y < c.cell[1] + side {
        let mut x = c.cell[0] - reach;
        while x < c.cell[0] + side {
            if x != c.cell[0] || y != c.cell[1] {
                let (d, here) = candidate_at(decor, [x, y]);
                if here && before(&d, c) && meet(&d, side_of(decor, &d), c, side) {
                    return false;
                }
            }
            x += 1;
        }
        y += 1;
    }
    true
}

/// What stands in leaf cell `cell`: its candidate, where it holds one and it
/// stands clear (`retained`).
pub fn scatter_at(decor: &Decor, cell: [i32; 2]) -> Option<Candidate> {
    let (c, here) = candidate_at(decor, cell);
    (here && retained(decor, &c)).then_some(c)
}

/// Whether `variant` of `decor` stands as itself in a window at `cell`, by
/// its own size, not the window's: at the leaves' always, the finest window
/// there is; at a coarser one where the brick it claims (its widest side, as
/// `side_of` measures it) is no narrower than the window's cell. Where it is
/// narrower it is less than a cell there, and stands in none coarser.
pub const fn stands_at(decor: &Decor, variant: u32, cell: Zoom) -> bool {
    stands(decor.sizes[variant as usize], decor.cell_zoom(), cell)
}

/// `stands_at` of a box `[x, y, _]` in cells at `own`, by value.
const fn stands([x, y, _]: [u32; 3], own: Zoom, cell: Zoom) -> bool {
    let side = if x > y { x } else { y };
    cell >= LEAF_CELL || own <= cell || (own - cell < u32::BITS && side >= 1 << (own - cell))
}

/// A folded share's unit, of `answer::FULL`.
pub const FOLD_UNIT: u32 = answer::FULL / 64;

/// The share of a window's cell at `cell` that `variant` of `decor` claims,
/// in `FOLD_UNIT`s, rounded: its box's room over the cell's cube.
/// None at its own cell or finer, where it is a cell or more.
pub const fn fold_share(decor: &Decor, variant: u32, cell: Zoom) -> u32 {
    share(decor.sizes[variant as usize], decor.cell_zoom(), cell)
}

/// `fold_share` of a box `[x, y, z]` in cells at `own`, by value.
const fn share([x, y, z]: [u32; 3], own: Zoom, cell: Zoom) -> u32 {
    if cell >= own {
        return 0;
    }
    // Each axis `own - cell` halvings narrower than the cell, the room
    // 8 times less a halving.
    let shift = 3 * (own - cell);
    let units = answer::FULL / FOLD_UNIT;
    if shift >= u32::BITS - 2 * units.trailing_zeros() {
        return 0;
    }
    (x * y * z * units + (1 << shift >> 1)) >> shift
}

/// Whether `variant` of `decor` is folded into a window's cell at `cell`:
/// where it does not stand as itself (`stands_at`), smaller than the cell,
/// the room it claims added to the cell's, as a cube's is to its parent's
/// (`ground::Cell::folded`), where that room is a unit of the share or more
/// (`fold_share`).
pub const fn folds_at(decor: &Decor, variant: u32, cell: Zoom) -> bool {
    !stands_at(decor, variant, cell) && fold_share(decor, variant, cell) > 0
}

/// Whether any variant of `decor` is folded at `cell` (`folds_at`).
pub const fn any_folds_at(decor: &Decor, cell: Zoom) -> bool {
    let mut v = 0;
    while v < decor.variants {
        if folds_at(decor, v, cell) {
            return true;
        }
        v += 1;
    }
    false
}

/// The coarsest window zoom each decor is folded at (`any_folds_at`),
/// `LEAF_CELL` where it is folded at none: past it its room rounds to none.
const FOLDS_FROM: [Zoom; DECORS.len()] = {
    let mut from = [LEAF_CELL; DECORS.len()];
    let mut k = 0;
    while k < DECORS.len() {
        let mut cell = LEAF_CELL;
        while cell > 0 && any_folds_at(&DECORS[k], cell - 1) {
            cell -= 1;
        }
        from[k] = cell;
        k += 1;
    }
    from
};

/// The decor folded into the cell `b` at `cell` whose ground's own look is
/// `look`: of each decor that stands on it, the room of every one the leaf
/// cells under it hold that is folded there (`folds_at`), added as a cube's
/// is, in `answer::FULL`'s units, no more than the cell; and the look it is
/// folded to, the greatest a decor names (`Decor::fold`), 0 for the
/// ground's. None at a leaf's cell or finer, and none of a decor coarser
/// than its room rounds to a unit at (`FOLDS_FROM`). Each row's words are
/// read by value, as a GPU holds no pointer into the catalog a loop moves.
#[inline(never)]
pub fn folded(cell: Zoom, b: [i32; 2], look: u32) -> (u32, u32) {
    let (mut share_cnt, mut folds) = (0, 0);
    if cell >= LEAF_CELL || look >= 256 {
        return (0, 0);
    }
    let s = LEAF_CELL - cell;
    let mut k = 0;
    while k < DECORS.len() {
        let (seed, chance, variants) = (DECORS[k].seed, DECORS[k].chance, DECORS[k].variants);
        let (on, own, fold) = (
            DECORS[k].on[(look >> 5) as usize],
            UNIT_ZOOM - DECORS[k].scale_steps as Zoom,
            DECORS[k].fold,
        );
        if on >> (look & 31) & 1 == 1 && cell >= FOLDS_FROM[k] {
            let mut i = 0;
            while i < 1 << (2 * s) {
                let leaf = [(b[0] << s) + (i & ((1 << s) - 1)), (b[1] << s) + (i >> s)];
                let (c, here) = drawn(seed, chance, leaf);
                let v = mul_hi(SplitMix32::new(c.seed).next_u32(), variants);
                let size = DECORS[k].sizes[v as usize];
                if here && !stands(size, own, cell) {
                    let n = share(size, own, cell);
                    share_cnt += n;
                    folds = if n > 0 && fold > folds { fold } else { folds };
                }
                i += 1;
            }
        }
        k += 1;
    }
    let held = share_cnt * FOLD_UNIT;
    (
        if held > answer::FULL {
            answer::FULL
        } else {
            held
        },
        folds,
    )
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

    /// Whether the ground generates it on a cell of `value`: a look it stands on.
    pub const fn allows(&self, val: u32) -> bool {
        val < 256 && self.on[(val >> 5) as usize] >> (val & 31) & 1 == 1
    }

    /// Its cell's zoom: a unit's, `scale_steps` coarser.
    pub const fn cell_zoom(&self) -> Zoom {
        (UNIT_ZOOM as i32 - self.scale_steps) as Zoom
    }
}

#[cfg(test)]
#[path = "scatter.test.rs"]
mod test;
