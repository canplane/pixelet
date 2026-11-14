// pxl-content/scatter.rs

//! wgsl-port: the decor the ground holds, never stored, as a tree of hashes. A brick at a
//! decor's root zoom (`ROOTS`) holds as many as one hash draws around its mean, and each
//! column hands its count down to its four children by weights near a half that one hash a
//! column draws (`near_half`), to the leaf cells, a unit each, which hold one or none (`cnt`). A leaf cell's own hash draws its
//! variant and heading (`candidate_at`); one whose box meets the box of a neighbor that drew
//! lower gives way to it (`retained`). At a zoom, a decor stands as itself where its box keeps
//! an eighth of that zoom's cell or more, as a folded cube is drawn, and is none where it
//! keeps none (`stands_at`), placed or generated alike. Every draw is a u32 (xpute-kit
//! `SplitMix32`), seeded by the decor's seed and the column (`node_seed`), so every reader, on
//! either side, draws the same.

use pxl_engine::clipmap::FIRST_CELL_ZOOM;
use pxl_engine::seed::node_seed;
use pxl_spec::brick::BRICK_SHIFT;
use pxl_spec::share::EIGHTH_CNT;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM, UNIT_ZOOM};
use xpute_kit::math::rng::SplitMix32;

use crate::halve::{self, weight};
use pxl_spec::asset::decor::{Row, ROWS};

/// A cell's candidate: the leaf cell it stands at, at every zoom; its first draw, which is
/// tested against the decor's chance and orders, lower first, which neighbors give way
/// (`before`); and the seed of its later draws (variant, heading).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub cell: [i32; 2],
    pub draw: u32,
    pub seed: u32,
}

/// Leaf cell `cell`'s candidate of the `k`-th decor, with its own draws.
pub fn candidate_at(k: usize, cell: [i32; 2]) -> Candidate {
    let mut draws = SplitMix32::new(node_seed(ROWS[k].seed, (LEAF_CELL_ZOOM, cell)));
    Candidate {
        cell,
        draw: draws.next_u32(),
        seed: draws.next_u32(),
    }
}

/// The tags of a column's count and of its split among its four, beside a
/// decor's seed (`node_seed`).
pub const CNT_TAG: u32 = 0xc047_dec0;
pub const SPLIT_TAG: u32 = 0x5b17_dec0;

/// How many of the `k`-th decor the column `b` at `zoom` holds; a leaf cell holds one or
/// none. Its root's column (`ROOTS`) holds as many as a hash draws around its mean, the
/// decor's chance for each leaf cell under it, rounded at a uniform draw. Each column's count
/// is then halved across and along, exactly, by weights its hash draws (`near_half`), with
/// none past what its leaf cells hold. Coarser than the root, it is none. Every word is read by value.
pub fn cnt(k: usize, zoom: Zoom, b: [i32; 2]) -> u32 {
    cnt_of([ROWS[k].seed, ROWS[k].chance, ROOTS[k]], zoom, b)
}

/// A hash's weight held within a sixteenth of a half, its low bits still the draw that rounds
/// (`halve`). Weights over the whole range would heap a brick's decor in a few cells and leave
/// the rest bare.
const fn near_half(w: u32) -> u32 {
    weight(0x7000 + (w >> 16 & 0x1fff), w)
}

/// The mean a column at `root` holds, in 16.16 fixed point: the chance's top half for each
/// leaf cell under it.
const fn mean(chance: u32, root: Zoom) -> u32 {
    (chance >> 16) << (2 * (LEAF_CELL_ZOOM - root))
}

/// The most of `decor` any brick holds: its count's root (`root`) is the brick at the
/// coarsest clip level it stands at, which holds its mean rounded (`cnt`), and a finer
/// clip level's brick holds a share of that.
pub const fn brick_cap(decor: &Row) -> u32 {
    (mean(decor.chance, root(decor)) + 0xffff) >> 16
}

/// `cnt` of a decor whose seed, chance and root are `seed`, `chance` and `root`.
pub fn cnt_of([seed, chance, root]: [u32; 3], zoom: Zoom, b: [i32; 2]) -> u32 {
    if zoom < root {
        return 0;
    }
    let at = |z: Zoom| [b[0] >> (zoom - z), b[1] >> (zoom - z)];
    let draw = node_seed(seed ^ CNT_TAG, (root, at(root)));
    let mut n = (mean(chance, root) + (draw & 0xffff)) >> 16;
    let mut z = root;
    while z < zoom && n > 0 {
        // Halved across, then along, none past what its leaf cells hold
        // (`halve::quarter`), the one rule the caves and the cumulus hand
        // theirs down by, each half along by a weight of its own.
        let mut draws = SplitMix32::new(node_seed(seed ^ SPLIT_TAG, (z, at(z))));
        let c = at(z + 1);
        let quarter = (c[0] & 1 | (c[1] & 1) << 1) as usize;
        let cap = 1u32 << (2 * (LEAF_CELL_ZOOM - z - 1));
        let cap = if cap > 0xffff { 0xffff } else { cap };
        let [across, along, other] = [draws.next_u32(), draws.next_u32(), draws.next_u32()];
        let along = if quarter & 1 == 0 { along } else { other };
        let weights = [near_half(across), near_half(along)];
        n = halve::quarter(n, cap, weights, quarter as u32);
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

/// Whether the bricks two candidates claim, `sa` and `sb` cells a side counting up from their
/// cells, share a cell.
pub fn meet(a: &Candidate, sa: i32, b: &Candidate, sb: i32) -> bool {
    a.cell[0] < b.cell[0] + sb
        && b.cell[0] < a.cell[0] + sa
        && a.cell[1] < b.cell[1] + sb
        && b.cell[1] < a.cell[1] + sa
}

/// The high word of `a × b`, in u32 alone.
pub(crate) const fn mul_hi(a: u32, b: u32) -> u32 {
    let (a0, a1, b0, b1) = (a & 0xffff, a >> 16, b & 0xffff, b >> 16);
    let (low, cross0, cross1) = (a0 * b0, a1 * b0, a0 * b1);
    let carry = (low >> 16) + (cross0 & 0xffff) + (cross1 & 0xffff);
    a1 * b1 + (cross0 >> 16) + (cross1 >> 16) + (carry >> 16)
}

/// The variant a candidate draws, its seed's first draw: uniform over the
/// decor's.
pub(crate) fn draw_variant(decor: &Row, draws: &mut SplitMix32) -> u32 {
    mul_hi(draws.next_u32(), decor.variant_cnt)
}

/// The variant a candidate of `seed` draws (`draw_variant`), the first of its draws.
pub fn variant_of(decor: &Row, seed: u32) -> u32 {
    draw_variant(decor, &mut SplitMix32::new(seed))
}

/// The brick a candidate claims, which holds its box however it turns.
pub fn side_of(decor: &Row, seed: u32) -> i32 {
    let [x, y, _] = decor.sizes[variant_of(decor, seed) as usize];
    x.max(y) as i32
}

/// Whether `c` stands clear of the `k`-th decor's other candidates: no candidate before it
/// (`before`) claims a brick meeting its own (`side_of`, `meet`), whether or not that one gives
/// way itself, as Matérn's second thinning keeps one. The cells asked are those at which a brick
/// meeting `c`'s may start, as wide as the decor's widest.
pub fn retained(k: usize, c: &Candidate) -> bool {
    let decor = &ROWS[k];
    let (side, extent) = (side_of(decor, c.seed), decor.widest() as i32 - 1);
    let mut y = c.cell[1] - extent;
    while y < c.cell[1] + side {
        let mut x = c.cell[0] - extent;
        while x < c.cell[0] + side {
            if (x != c.cell[0] || y != c.cell[1]) && cnt(k, LEAF_CELL_ZOOM, [x, y]) > 0 {
                let d = candidate_at(k, [x, y]);
                if before(&d, c) && meet(&d, side_of(decor, d.seed), c, side) {
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
pub fn scatter_at(k: usize, cell: [i32; 2]) -> Option<Candidate> {
    (cnt(k, LEAF_CELL_ZOOM, cell) > 0)
        .then(|| candidate_at(k, cell))
        .filter(|c| retained(k, c))
}

/// Whether `variant` of `decor` stands as itself in a clip level at `cell_zoom`, judged by its
/// own size, not the clip level's. It always stands at the leaves', the finest clip level
/// there is. At a coarser one it stands where its box, in one of the clip level's cells, keeps
/// an eighth of the cell or more, floored as a folded cube's share is (`share::eighths`), so
/// a decor stands at the zooms at which a cube of its box is drawn (`stands`). Where it keeps
/// none, it is none there and at every coarser zoom.
pub const fn stands_at(decor: &Row, variant: u32, cell_zoom: Zoom) -> bool {
    stands(decor.sizes[variant as usize], UNIT_ZOOM, cell_zoom)
}

/// `stands_at` for a box `[x, y, z]` in cells at `own`, by value: whether the eighths of a
/// cell at `cell_zoom` that its volume fills, floored (`share::eighths`), are one or more.
const fn stands([x, y, z]: [u32; 3], own: Zoom, cell_zoom: Zoom) -> bool {
    let steps = 3 * (own - if own < cell_zoom { own } else { cell_zoom });
    cell_zoom >= LEAF_CELL_ZOOM || (steps < u32::BITS && (EIGHTH_CNT * x * y * z) >> steps >= 1)
}

/// The coarsest clip level zoom `variant` of `decor` stands at (`stands_at`).
pub const fn coarsest_stand(decor: &Row, variant: u32) -> Zoom {
    let mut cell_zoom = LEAF_CELL_ZOOM;
    while cell_zoom > 0 && stands_at(decor, variant, cell_zoom - 1) {
        cell_zoom -= 1;
    }
    cell_zoom
}

/// The coarsest clip level zoom any variant of `decor` stands at (`coarsest_stand`): it stands
/// at this zoom and every finer one.
pub const fn first_cell_zoom(decor: &Row) -> Zoom {
    let (mut first, mut v) = (LEAF_CELL_ZOOM, 0);
    while v < decor.variant_cnt {
        let z = coarsest_stand(decor, v);
        first = if z < first { z } else { first };
        v += 1;
    }
    first
}

/// The zoom a decor's counts start at: that of a brick at the coarsest clip level it stands
/// at (`first_cell_zoom`).
pub const fn root(decor: &Row) -> Zoom {
    let root = first_cell_zoom(decor);
    (if root > FIRST_CELL_ZOOM {
        root
    } else {
        FIRST_CELL_ZOOM
    }) - BRICK_SHIFT
}

/// The coarsest clip level zoom any decor stands at (`first_cell_zoom`).
pub const COARSEST_STAND: Zoom = {
    let (mut coarsest, mut k) = (LEAF_CELL_ZOOM, 0);
    while k < ROWS.len() {
        let z = first_cell_zoom(&ROWS[k]);
        coarsest = if z < coarsest { z } else { coarsest };
        k += 1;
    }
    coarsest
};

/// The root of each decor's counts (`cnt`, `root`).
pub const ROOTS: [Zoom; ROWS.len()] = {
    let mut roots = [LEAF_CELL_ZOOM; ROWS.len()];
    let mut k = 0;
    while k < ROWS.len() {
        roots[k] = root(&ROWS[k]);
        k += 1;
    }
    roots
};
const _: () = {
    let mut k = 0;
    while k < ROWS.len() {
        let shift = 2 * (LEAF_CELL_ZOOM - ROOTS[k]);
        assert!(
            shift < u32::BITS,
            "a decor's root column's leaf cells, a shift, fit a u32 (`cnt`)"
        );
        let mean = ((ROWS[k].chance >> 16) as u64) << shift;
        assert!(
            mean + 0xffff <= u32::MAX as u64,
            "a decor's mean a root column, in 16.16, fits a u32 (`cnt`)"
        );
        k += 1;
    }
};

#[cfg(test)]
#[path = "scatter.test.rs"]
mod test;
