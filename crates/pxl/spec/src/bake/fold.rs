// pxl-spec/bake/fold.rs

//! One level up: a parent page from the pages under it. The bake runs it to
//! build the pyramid, and the client runs it to stand a coarse page up from
//! fine ones it already holds, so it lives here, once. Both sides compose the
//! same one-level function the same number of times, which is why any rule
//! that is deterministic agrees between them — the rule does not also have to
//! be a homomorphism (issue/20260921-level-step-and-zoom-are-one-number.md).
//!
//! A parent covers 2×2 children at half the resolution: a parent cell is the
//! 2×2 child cells under it. Ground takes the type that fills at least half of
//! them, so a feature narrower than half a cell of the level below rounds away
//! and the channel under it shows. Nothing here knows which channel it is
//! folding: what disappears at a distance is a matter of size, not a policy
//! per layer.
//!
//! Half of the level below, not of the leaves. A mode is holistic — no
//! summary of fixed size gives the mode of the union — so a mode of modes is
//! not the mode of the block: a line one leaf wide can win a tie at level 1,
//! be a whole cell there, and win the tie again at level 2 at a quarter of
//! its cell; a type filling 27 of 64 leaves in three quarters of three
//! quarters wins at level 3. Earth Engine's `MODE` pyramiding policy makes the
//! same trade — each level the mode of 2×2 of the level below, the upper-left
//! pixel when there is none — and for the same reason: a level that is a
//! function of the one under it is what lets the client fold what it holds.
//! Exact would mean per-type counts through every level, 32 lanes a cell,
//! which the resident records cannot carry. Where this differs from Earth
//! Engine is the tie's corner (`tie_child`).
//!
//! Elevation is the box average, and that is forced rather than chosen. A
//! column's volume is linear in its height, so the mean is what preserves it,
//! and the fraction the mean leaves is what a coarse cell's inset is cut from.
//! The value stays in units at every level: in the level's own cells the
//! rounding would eat that fraction.

use xpute_core::math::scalar::round;

use super::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_CELL_SPAN, PAGE_ELE_SPAN};
use super::format::{Ground, Terrain};

/// Which child of the 2×2 a tie takes when folding into `level`.
///
/// Always the same child would put every tie's sample at the block's south-west
/// corner, which shifts the whole map half a coarse cell toward the origin and
/// accumulates level by level. Alternating the corner keeps the drift bounded
/// by a sixth of a cell.
fn tie_child(level: u32) -> usize {
    if level % 2 == 1 {
        0
    } else {
        3
    }
}

/// The type that fills at least half of four, or `tie` when none does or two
/// fill half each.
fn majority(v: [u8; 4], tie: u8) -> u8 {
    let count = |x: u8| v.iter().filter(|&&y| y == x).count();
    let mut half = None;
    for &x in &v {
        match count(x) {
            n if n >= 3 => return x,
            2 if half.is_some_and(|h| h != x) => return tie,
            2 => half = Some(x),
            _ => {}
        }
    }
    half.unwrap_or(tie)
}

/// The same rule one dimension up, for what is a volume rather than a height
/// field: the value that fills at least half of eight, or the tie child's
/// when none does or two fill half each. Empty is a value like any other, so
/// a cube filling an eighth of its parent goes at the first fold and a wall
/// keeps whole cells. `level` is the parent's, and may be below the cell: an
/// object's finest entries are an eighth of one. `v[dx + 2·dy + 4·dz]` is the
/// child at (dx, dy, dz).
pub fn majority8(v: [u32; 8], level: i32) -> u32 {
    // The 2×2 alternation, on the diagonal a level up: the south-west-lowest
    // corner and its opposite, so the drift stays bounded on every axis.
    let tie = v[if level.rem_euclid(2) == 1 { 0 } else { 7 }];
    let count = |x: u32| v.iter().filter(|&&y| y == x).count();
    let mut half = None;
    for &x in &v {
        match count(x) {
            n if n >= 5 => return x,
            4 if half.is_some_and(|h| h != x) => return tie,
            4 => half = Some(x),
            _ => {}
        }
    }
    half.unwrap_or(tie)
}

/// One plane folded: `children[dx + 2·dy]` is the child at (dx, dy) of the
/// parent, each `PAGE_CELL_SPAN²` cells, and `out` the parent's plane.
pub fn fold_plane(children: [&[u8]; 4], level: u32, out: &mut [u8]) {
    let span = PAGE_CELL_SPAN as usize;
    let half = span / 2;
    let tie = tie_child(level);
    for py in 0..span {
        let (dy, cy) = (py / half, (2 * py) % span);
        for px in 0..span {
            let (dx, cx) = (px / half, (2 * px) % span);
            let c = children[dx + 2 * dy];
            let at = cy * span + cx;
            let v = [c[at], c[at + 1], c[at + span], c[at + span + 1]];
            out[py * span + px] = majority(v, v[tie]);
        }
    }
}

/// Every ground channel folded the same way.
pub fn fold_ground(children: [&Ground; 4], level: u32) -> Ground {
    let plane = |pick: fn(&Ground) -> &[u8]| {
        let mut out = vec![0u8; (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize];
        fold_plane(children.map(pick), level, &mut out);
        out.into()
    };
    Ground {
        land: plane(|g| &g.land),
        water: plane(|g| &g.water),
        structure: plane(|g| &g.structure),
        transport: plane(|g| &g.transport),
    }
}

/// The elevation folded: `around[(qx + 1) + 4·(qy + 1)]` is the padded grid of
/// the child at (qx, qy) for qx, qy in −1..=2, the parent's own four being
/// 0..=1. A page that does not exist is the ocean's, which is what it would
/// have been baked as.
///
/// The ring of neighbours is what the skirt costs. A parent's two padding
/// samples reach four child samples past the block, and a child pads only two,
/// so they are read from the neighbours' own inner samples — never from a
/// child's padding, which would read a copy of the same thing and take the
/// fold's answer from how that copy was made.
///
/// A ring page may be `None`, which is the client folding from what it holds
/// rather than the bake folding a whole pyramid. Where one is, the block's
/// own four answer instead: they pad two samples of their own, which *are*
/// the neighbour's samples and reach half as far as the parent's skirt, and
/// past that the block's edge is extended. Nothing inside the parent moves —
/// every sample of it is read from the four — so a page folded without the
/// ring differs from the baked one in its outermost padding sample alone,
/// which is the fourth tap of a bicubic read at the page's own rim.
pub fn fold_ele(around: [Option<&[i16]>; 16], out: &mut [i16]) {
    let inner = PAGE_ELE_SPAN as i64;
    let (pad, padded) = (ELE_PAD as i64, ELE_PADDED_SPAN as usize);
    let read = |qx: i64, qy: i64, mx: i64, my: i64| -> Option<i64> {
        let grid = around[((qx + 1) + 4 * (qy + 1)) as usize]?;
        let (ix, iy) = ((mx - qx * inner + pad) as usize, (my - qy * inner + pad) as usize);
        Some(grid[iy * padded + ix] as i64)
    };
    // As far past the block as the four children's own padding reaches.
    let of_block = |mx: i64, my: i64| -> Option<i64> {
        let near = |m: i64| m.clamp(-pad, 2 * inner - 1 + pad);
        let (mx, my) = (near(mx), near(my));
        let q = |m: i64| m.div_euclid(inner).clamp(0, 1);
        read(q(mx), q(my), mx, my)
    };
    // One child sample in the parent's 2×2 block, by its index across the
    // block's two pages (−2·pad .. 2·inner + 2·pad).
    let sample = |mx: i64, my: i64| -> i64 {
        let (qx, qy) = (mx.div_euclid(inner), my.div_euclid(inner));
        read(qx, qy, mx, my).or_else(|| of_block(mx, my)).unwrap_or(0)
    };
    for sy in 0..padded {
        let iy = sy as i64 - pad;
        for sx in 0..padded {
            let ix = sx as i64 - pad;
            let (mx, my) = (2 * ix, 2 * iy);
            let sum = sample(mx, my) + sample(mx + 1, my) + sample(mx, my + 1) + sample(mx + 1, my + 1);
            out[sy * padded + sx] = round(sum as f64 / 4.0) as i16;
        }
    }
}

/// A parent page from the 4×4 pages around its four children, indexed as
/// `fold_ele` takes them. Ground reads only the middle four; elevation reads
/// all sixteen.
pub fn fold_terrain(around: [&Terrain; 16], level: u32) -> Terrain {
    fold_terrain_held(around.map(Some), level).expect("every page of the block")
}

/// A parent page from whichever of those sixteen are held. The middle four —
/// 5, 6, 9 and 10, the parent's own children — are what it is made of and
/// None in any of them is None here; the ring is the skirt's and `fold_ele`
/// says what its absence costs.
///
/// This is why a zoom out need not reach the network. The client holds the
/// level it is leaving, and a level is folded from four pages of it, not
/// from the sixteen a bake has lying around.
pub fn fold_terrain_held(around: [Option<&Terrain>; 16], level: u32) -> Option<Terrain> {
    let child = |k: usize| around[k].map(|t| &t.ground);
    let ground = fold_ground([child(5)?, child(6)?, child(9)?, child(10)?], level);
    let mut ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    Some(Terrain { ground, ele: ele.into() })
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
