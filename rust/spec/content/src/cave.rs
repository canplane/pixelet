// pxl-content/cave.rs

//! gpu-portable: the caves, u32 hashes over i32 cells (pxl-engine
//! `seed::node_seed`), no `Option<bool>`, and each lattice corner by itself.
//!
//! The caves are Minecraft 1.18's noise caves: tunnels where two noises are
//! both near zero, its spaghetti, and caverns where one is high, its cheese,
//! a finer noise over all three so no wall runs smooth. Each noise is Ken
//! Perlin's improved noise on a lattice of its own, a hash at each lattice
//! point (`node_seed`) choosing its gradient, all in integers, so the CPU
//! and a GPU carve the same cells to the bit. A cave is a pure function of
//! the cube and the seed.

use pxl_engine::seed::node_seed;
use pxl_engine::{idx_at, wrap_idx, Zoom, LEAF_CELL};

use crate::param::cave;

/// The coarsest cube a cave takes, 32 units a side: nothing coarser is
/// hollow, so every coarser fold of the caves is solid.
pub const CAVE_COARSEST: Zoom = LEAF_CELL - cave::COARSEST_STEPS;

/// Where a point lies in a lattice cell, in fixed point: `T_ONE` across it.
const T_BITS: u32 = 14;
const T_ONE: i32 = 1 << T_BITS;

/// Ken Perlin's quintic fade, 6t⁵ − 15t⁴ + 10t³, over `[0, T_ONE]`, in
/// steps that stay within an i32. Their floors make it fall back a little
/// where it rises slowest, never more than `FADE_DROP` under a value it took
/// before (cave.test.rs).
fn fade(t: i32) -> i32 {
    let t3 = (((t * t) >> T_BITS) * t) >> T_BITS;
    let inner = 10 * T_ONE + ((((6 * t - 15 * T_ONE) >> 2) * t) >> (T_BITS - 2));
    (t3 * (inner >> 2)) >> (T_BITS - 2)
}

/// The most `fade` falls under a value it took at a smaller `t`.
const FADE_DROP: i32 = 7;

/// `a` toward `b` by `t` of `T_ONE`, never past either: the floor of a
/// product no larger than either's span.
fn lerp(a: i32, b: i32, t: i32) -> i32 {
    a + (((b - a) * t) >> T_BITS)
}

/// A lattice: its tag (`node_seed`), and its spacing as powers of two of
/// units, across (X and Y) and up (Z). The tunnels' two noises come off one
/// hash, the detail's three off another.
#[derive(Clone, Copy)]
struct Lattice {
    tag: u32,
    shift: [u32; 2],
}

const TUNNEL_LATTICE: Lattice = Lattice {
    tag: 0xca7e_0001,
    shift: cave::TUNNEL_SHIFT,
};
const CAVERN_LATTICE: Lattice = Lattice {
    tag: 0xca7e_0002,
    shift: cave::CAVERN_SHIFT,
};
const DETAIL_LATTICE: Lattice = Lattice {
    tag: 0xca7e_0003,
    shift: cave::DETAIL_SHIFT,
};

impl Lattice {
    /// The lattice cell `p`, in half units, lies in, and where in it along
    /// each axis, in `T_ONE`.
    fn place(self, p: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let (x, y, z) = (
            along(p[0], self.shift[0]),
            along(p[1], self.shift[0]),
            along(p[2], self.shift[1]),
        );
        ([x.0, y.0, z.0], [x.1, y.1, z.1])
    }

    /// The hashes at the eight corners of the lattice cell `cell`, corner
    /// `dx + 2·dy + 4·dz`. X is wrapped as the world wraps it, its lattice's
    /// zoom the one whose cells are its spacing.
    fn corners(self, [x, y, z]: [i32; 3]) -> [u32; 8] {
        let at = (LEAF_CELL - self.shift[0], self.tag);
        [
            node_seed(at.1, (at.0, [x, y, z])),
            node_seed(at.1, (at.0, [x + 1, y, z])),
            node_seed(at.1, (at.0, [x, y + 1, z])),
            node_seed(at.1, (at.0, [x + 1, y + 1, z])),
            node_seed(at.1, (at.0, [x, y, z + 1])),
            node_seed(at.1, (at.0, [x + 1, y, z + 1])),
            node_seed(at.1, (at.0, [x, y + 1, z + 1])),
            node_seed(at.1, (at.0, [x + 1, y + 1, z + 1])),
        ]
    }
}

/// Along one axis, the lattice cell `p`, in half units (`HALF_UNIT`), lies
/// in, its spacing `2^shift` units, and where in it, in `T_ONE`.
fn along(p: i32, shift: u32) -> (i32, i32) {
    let s = shift + 1;
    (
        idx_at(HALF_UNIT, p, LEAF_CELL - shift),
        (p & ((1 << s) - 1)) << (T_BITS - s),
    )
}

/// The zoom whose cells are half a unit, the points a cube's center and its
/// corners fall on.
const HALF_UNIT: Zoom = LEAF_CELL + 1;

/// Ken Perlin's improved gradient, the low four bits of `h`, one of the
/// cube's twelve edges, four of them twice: its least and most against the
/// offsets `[lo, hi]` from its lattice point, in `T_ONE`, each axis its own.
fn grad_span(h: u32, lo: [i32; 3], hi: [i32; 3]) -> [i32; 2] {
    let h = h & 15;
    let (u0, u1) = if h < 8 {
        (lo[0], hi[0])
    } else {
        (lo[1], hi[1])
    };
    let (v0, v1) = if h < 4 {
        (lo[1], hi[1])
    } else if h == 12 || h == 14 {
        (lo[0], hi[0])
    } else {
        (lo[2], hi[2])
    };
    let (u0, u1) = if h & 1 == 0 { (u0, u1) } else { (-u1, -u0) };
    let (v0, v1) = if h & 2 == 0 { (v0, v1) } else { (-v1, -v0) };
    [u0 + v0, u1 + v1]
}

/// Corner `k`'s offset from the point at `t` in its lattice cell.
fn offset(k: usize, t: [i32; 3]) -> [i32; 3] {
    [
        t[0] - (k & 1) as i32 * T_ONE,
        t[1] - (k >> 1 & 1) as i32 * T_ONE,
        t[2] - (k >> 2 & 1) as i32 * T_ONE,
    ]
}

/// `lerp`'s least over `a` and `b` at their least and `t` between `t0` and
/// `t1`, and its most at their most: it rises with `a` and `b`, and runs
/// straight in `t`.
fn lerp_span(a: [i32; 2], b: [i32; 2], t0: i32, t1: i32) -> [i32; 2] {
    let (l0, l1) = (lerp(a[0], b[0], t0), lerp(a[0], b[0], t1));
    let (m0, m1) = (lerp(a[1], b[1], t0), lerp(a[1], b[1], t1));
    [if l0 < l1 { l0 } else { l1 }, if m0 > m1 { m0 } else { m1 }]
}

/// The least and the most the noise whose gradients are `h`'s bits from
/// `from` takes over the box `[lo, hi]` of its lattice cell, in `T_ONE`,
/// about `±T_ONE`: each corner's gradient over the box, blended over the
/// fades the box's points take, from its first end's less `FADE_DROP` to its
/// last's and `FADE_DROP` more. Over a box of no size, a point, it is the
/// noise there, both ends.
fn noise_span(h: [u32; 8], from: u32, [lo, hi]: [[i32; 3]; 2]) -> [i32; 2] {
    let g = |k: usize, h: u32| grad_span(h >> from, offset(k, lo), offset(k, hi));
    let v = [
        g(0, h[0]),
        g(1, h[1]),
        g(2, h[2]),
        g(3, h[3]),
        g(4, h[4]),
        g(5, h[5]),
        g(6, h[6]),
        g(7, h[7]),
    ];
    let below = |t: i32, u: i32| {
        let f = fade(t) - if u > t { FADE_DROP } else { 0 };
        if f < 0 {
            0
        } else {
            f
        }
    };
    let above = |t: i32, u: i32| {
        let f = fade(u) + if u > t { FADE_DROP } else { 0 };
        if f > T_ONE {
            T_ONE
        } else {
            f
        }
    };
    let f0 = [
        below(lo[0], hi[0]),
        below(lo[1], hi[1]),
        below(lo[2], hi[2]),
    ];
    let f1 = [
        above(lo[0], hi[0]),
        above(lo[1], hi[1]),
        above(lo[2], hi[2]),
    ];
    let x00 = lerp_span(v[0], v[1], f0[0], f1[0]);
    let x10 = lerp_span(v[2], v[3], f0[0], f1[0]);
    let x01 = lerp_span(v[4], v[5], f0[0], f1[0]);
    let x11 = lerp_span(v[6], v[7], f0[0], f1[0]);
    let y0 = lerp_span(x00, x10, f0[1], f1[1]);
    let y1 = lerp_span(x01, x11, f0[1], f1[1]);
    lerp_span(y0, y1, f0[2], f1[2])
}

/// The detail over a noise: a share of it (`cave::DETAIL_SHRINK`).
fn detail(n: i32) -> i32 {
    n >> cave::DETAIL_SHRINK
}

/// The leaf cell a cube finer than the leaves lies in, and any other as
/// itself, its X wrapped as the world wraps it.
fn leaf_or_coarser((zoom, c): (Zoom, [i32; 3])) -> (Zoom, [i32; 3]) {
    let to = if zoom > LEAF_CELL { LEAF_CELL } else { zoom };
    let [x, y, z] = c;
    (
        to,
        [
            wrap_idx(to, idx_at(zoom, x, to)),
            idx_at(zoom, y, to),
            idx_at(zoom, z, to),
        ],
    )
}

/// Whether the tunnels' two noises, `a` and `b` with the detail over them,
/// are both within the band about zero.
fn in_tunnel(a: i32, b: i32) -> bool {
    a > -cave::TUNNEL_BAND
        && a < cave::TUNNEL_BAND
        && b > -cave::TUNNEL_BAND
        && b < cave::TUNNEL_BAND
}

/// The box `[lo, hi]`, in `T_ONE`, that the cube `c` at `zoom`, no finer
/// than a leaf, takes of the lattice cell it lies in, and that cell: the
/// cube whole where `whole`, none (`false`) where it is larger than a
/// lattice cell along an axis; else its center, a box of no size.
fn span(
    lattice: Lattice,
    (zoom, c): (Zoom, [i32; 3]),
    whole: bool,
) -> ([i32; 3], [[i32; 3]; 2], bool) {
    let s = LEAF_CELL - zoom;
    let [across, up] = lattice.shift;
    if whole && (s > across || s > up) {
        return ([0; 3], [[0; 3]; 2], false);
    }
    // Its first corner, and its center half its side on.
    let half = if whole { 0 } else { 1 << s };
    let [x, y, z] = c;
    let lo = [
        idx_at(zoom, x, HALF_UNIT) + half,
        idx_at(zoom, y, HALF_UNIT) + half,
        idx_at(zoom, z, HALF_UNIT) + half,
    ];
    let (cell, lo) = lattice.place(lo);
    let hi = if whole {
        let (a, u) = (1 << (T_BITS + s - across), 1 << (T_BITS + s - up));
        [lo[0] + a, lo[1] + a, lo[2] + u]
    } else {
        lo
    };
    (cell, [lo, hi], true)
}

/// Whether the cube `c` at `zoom` is a cave's air (`whole` false), by the
/// noises at its center, or whether a cave takes all of it (`whole`),
/// whatever cube under it is asked, by the noises' least and most over it
/// (`noise_span`), known only where it lies in one cell of the detail's
/// lattice, the finest: in a tunnel, or in a cavern. A cube coarser than the
/// caves' first is solid; one finer than the leaves is its leaf's, whole.
/// One body for both, so a GPU's module writes it once.
fn caves(node: (Zoom, [i32; 3]), whole: bool) -> bool {
    let whole = whole && node.0 < LEAF_CELL;
    let (zoom, c) = leaf_or_coarser(node);
    if zoom < CAVE_COARSEST {
        return false;
    }
    let (cell, b, within) = span(DETAIL_LATTICE, (zoom, c), whole);
    if !within {
        return false;
    }
    let h = DETAIL_LATTICE.corners(cell);
    let da = noise_span(h, 0, b);
    let db = noise_span(h, 4, b);
    let dc = noise_span(h, 8, b);
    let (cell, b, _) = span(TUNNEL_LATTICE, (zoom, c), whole);
    let h = TUNNEL_LATTICE.corners(cell);
    let ta = noise_span(h, 0, b);
    let tb = noise_span(h, 4, b);
    if in_tunnel(ta[0] + detail(da[0]), tb[0] + detail(db[0]))
        && in_tunnel(ta[1] + detail(da[1]), tb[1] + detail(db[1]))
    {
        return true;
    }
    let (cell, b, _) = span(CAVERN_LATTICE, (zoom, c), whole);
    noise_span(CAVERN_LATTICE.corners(cell), 0, b)[0] + detail(dc[0]) > cave::CAVERN_OVER
}

/// Whether the cube `c` at `zoom` is a cave's air, by the noises at its
/// center (`caves`).
pub fn hollow(node: (Zoom, [i32; 3])) -> bool {
    caves(node, false)
}

/// Whether a cave takes all of the cube `c` at `zoom`, whatever cube under
/// it is asked (`caves`).
pub fn all_hollow(node: (Zoom, [i32; 3])) -> bool {
    caves(node, true)
}

const _: () = assert!(
    cave::DETAIL_SHIFT[0] <= cave::TUNNEL_SHIFT[0]
        && cave::DETAIL_SHIFT[1] <= cave::TUNNEL_SHIFT[1]
        && cave::DETAIL_SHIFT[0] <= cave::CAVERN_SHIFT[0]
        && cave::DETAIL_SHIFT[1] <= cave::CAVERN_SHIFT[1],
    "the detail's lattice is the finest, so a cube within one of its cells is within one of each"
);
const _: () = assert!(
    cave::TUNNEL_SHIFT[0] < T_BITS
        && cave::TUNNEL_SHIFT[1] < T_BITS
        && cave::CAVERN_SHIFT[0] < T_BITS
        && cave::CAVERN_SHIFT[1] < T_BITS,
    "a lattice cell's half units fit `T_ONE`"
);

#[cfg(test)]
#[path = "cave.test.rs"]
mod test;
