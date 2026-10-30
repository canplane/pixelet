// pxl-spec-core/scatter.rs

//! gpu-portable: the decor the ground holds, never stored, a tree of hashes
//! as the caves are: a column at a decor's root zoom (`ROOTS`) holds as many
//! as one hash draws about its mean, and each column hands its count down to
//! its four, one hash a column, to the leaf cells, a unit, which hold one or
//! none (`count`). A leaf cell's own hash draws its variant and heading
//! (`candidate_at`); one whose box meets a neighbor's that drew lower gives
//! way to it (`retained`). At a zoom a decor stands as itself where its box
//! keeps an eighth of that zoom's cell or more, as a folded cube is drawn, and
//! is none where it keeps none (`stands_at`), placed or generated alike.
//! Every draw is a u32 (xpute-kit `SplitMix32`), seeded by the decor's seed
//! and the column (`node_seed`), so every reader, either side, draws the
//! same.

use xpute_kit::math::rng::SplitMix32;

use crate::answer::EIGHTHS;
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

/// Leaf cell `cell`'s candidate of `decor`, its own draws, and whether the
/// cell holds one (`count`).
pub fn candidate_at(decor: &Decor, cell: [i32; 2]) -> (Candidate, bool) {
    let k = kind_of(decor.val);
    let mut draws = SplitMix32::new(node_seed(SEEDS[k], (LEAF_CELL, cell)));
    (
        Candidate {
            cell,
            draw: draws.next_u32(),
            seed: draws.next_u32(),
        },
        count(k, LEAF_CELL, cell) > 0,
    )
}

/// The first decor's stored value, the rest in the catalog's order after it.
const FIRST_VAL: u32 = DECORS[0].val;
const _: () = {
    let mut k = 0;
    while k < DECORS.len() {
        assert!(
            DECORS[k].val == FIRST_VAL + k as u32,
            "the decors' stored values run in the catalog's order"
        );
        k += 1;
    }
};

/// Each decor's words the hashes read, a small table each, so a GPU's lane
/// that reads one by a decor's place copies no row of the catalog: its seed
/// and its chance.
const SEEDS: [u32; DECORS.len()] = {
    let mut t = [0; DECORS.len()];
    let mut k = 0;
    while k < DECORS.len() {
        t[k] = DECORS[k].seed;
        k += 1;
    }
    t
};
const CHANCES: [u32; DECORS.len()] = {
    let mut t = [0; DECORS.len()];
    let mut k = 0;
    while k < DECORS.len() {
        t[k] = DECORS[k].chance;
        k += 1;
    }
    t
};
/// Whether `val` is a decor's stored value: a range, no row read, so a GPU's
/// lane asks nothing of the catalog its uniform control flow depends on.
pub const fn is_decor(val: u32) -> bool {
    val >= FIRST_VAL && val < FIRST_VAL + DECORS.len() as u32
}

/// Where the decor stored as `val` stands among the catalog's rows.
pub const fn kind_of(val: u32) -> usize {
    if is_decor(val) {
        (val - FIRST_VAL) as usize
    } else {
        0
    }
}

/// The tags of a column's count and of its split among its four, beside a
/// decor's seed (`node_seed`).
const COUNT: u32 = 0xc047_dec0;
const SPLIT: u32 = 0x5b17_dec0;

/// How many of the `k`-th decor the column `b` at `zoom` holds, a leaf
/// cell's one or none: its root's column (`ROOTS`) as many as a hash draws
/// about its mean, the decor's chance a leaf cell under it, rounded at a
/// uniform draw; then each column's count handed to its four a hash each,
/// each one to the quarter a draw names, none past what its leaf cells
/// hold. None coarser than the root. Every word read by value.
pub fn count(k: usize, zoom: Zoom, b: [i32; 2]) -> u32 {
    let (seed, chance, root) = (SEEDS[k], CHANCES[k], ROOTS[k]);
    if zoom < root {
        return 0;
    }
    let at = |z: Zoom| [b[0] >> (zoom - z), b[1] >> (zoom - z)];
    // The mean in 16.16, the chance's top half a leaf cell.
    let mean = (chance >> 16) << (2 * (LEAF_CELL - root));
    let draw = node_seed(seed ^ COUNT, (root, at(root)));
    let mut n = (mean + (draw & 0xffff)) >> 16;
    let mut z = root;
    while z < zoom && n > 0 {
        let mut draws = SplitMix32::new(node_seed(seed ^ SPLIT, (z, at(z))));
        let c = at(z + 1);
        let quarter = (c[0] & 1 | (c[1] & 1) << 1) as u32;
        let (mut mine, mut i) = (0, 0);
        while i < n {
            mine += u32::from(draws.next_u32() >> 30 == quarter);
            i += 1;
        }
        let room = 1 << (2 * (LEAF_CELL - z - 1));
        n = if mine < room { mine } else { room };
        z += 1;
    }
    n
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
/// there is; at a coarser one where its box, in one of the window's cells,
/// keeps an eighth of it or more, floored as a folded cube's share is
/// (`answer::eighths`), so a decor stands at the zooms a cube of its box is
/// drawn at (`stands`). Where it keeps none it is none there, and in none
/// coarser.
pub const fn stands_at(decor: &Decor, variant: u32, cell: Zoom) -> bool {
    stands(decor.sizes[variant as usize], decor.cell_zoom(), cell)
}

/// `stands_at` of a box `[x, y, z]` in cells at `own`, by value: the eighths
/// of a cell at `cell` its volume fills, floored (`answer::eighths`), one or
/// more.
const fn stands([x, y, z]: [u32; 3], own: Zoom, cell: Zoom) -> bool {
    let steps = 3 * (own - if own < cell { own } else { cell });
    cell >= LEAF_CELL || (steps < u32::BITS && (EIGHTHS * x * y * z) >> steps >= 1)
}

/// The coarsest window zoom `variant` of `decor` stands at (`stands_at`).
pub const fn coarsest_stand(decor: &Decor, variant: u32) -> Zoom {
    let mut cell = LEAF_CELL;
    while cell > 0 && stands_at(decor, variant, cell - 1) {
        cell -= 1;
    }
    cell
}

/// Each decor's counts' root (`count`): the coarsest window one of its
/// variants stands at, where a window first reads its columns.
pub const ROOTS: [Zoom; DECORS.len()] = {
    let mut roots = [LEAF_CELL; DECORS.len()];
    let mut k = 0;
    while k < DECORS.len() {
        let d = &DECORS[k];
        let mut cell = LEAF_CELL;
        while cell > 0 && any_stands_at(d, cell - 1) {
            cell -= 1;
        }
        roots[k] = cell;
        k += 1;
    }
    roots
};

/// Whether any variant of `decor` stands at `cell` (`stands_at`).
pub const fn any_stands_at(decor: &Decor, cell: Zoom) -> bool {
    let mut v = 0;
    while v < decor.variants {
        if stands_at(decor, v, cell) {
            return true;
        }
        v += 1;
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
