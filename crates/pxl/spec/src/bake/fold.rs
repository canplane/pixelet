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

use crate::space::pyramid::PAGE_CELLS;
use xpute_core::math::scalar::round;

use super::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
use super::format::{Ground, Terrain, GROUND_CH_KEYS};
use super::pack::{channel_of, with_channel, GroundCell};
use crate::space::pyramid::{neighbourhood, Folds, LevelPage, CHILDREN, GROUND};

/// The value that fills more than half of a 2×2 block, or of a 2×2×2 one,
/// folding into `level`; when none does, or two fill half each, the tie
/// child's. Empty is a value like any other, so a cube filling an eighth of
/// its parent goes at the first fold and a wall keeps whole cells. `v[dx +
/// 2·dy + 4·dz]` is the child at (dx, dy, dz), and `level` is the parent's,
/// which may be below the cell: an object's finest entries are an eighth of
/// one.
///
/// Always the same tie child would put every tie's sample at the block's
/// south-west(-lowest) corner, which shifts the whole map half a coarse cell
/// toward the origin and accumulates level by level. Alternating it with the
/// opposite corner keeps the drift bounded by a sixth of a cell on every
/// axis.
pub fn majority<T: Copy + Eq, const N: usize>(v: [T; N], level: i32) -> T {
    let tie = v[if level.rem_euclid(2) == 1 { 0 } else { N - 1 }];
    let count = |x: T| v.iter().filter(|&&y| y == x).count();
    let mut half = None;
    for &x in &v {
        match count(x) {
            n if 2 * n > N => return x,
            n if 2 * n == N && half.is_some_and(|h| h != x) => return tie,
            n if 2 * n == N => half = Some(x),
            _ => {}
        }
    }
    half.unwrap_or(tie)
}

/// Every ground channel folded the same way, each on its own: a channel votes
/// among its children's values for that channel and no other's, which is what
/// keeps a road from taking the land's vote under it.
///
/// Read straight out of the packed cells and written straight back, a field
/// at a time. A page is one array and there are no planes to take apart.
pub fn fold_ground(children: [&Ground; 4], level: u32) -> Ground {
    let out = (0..PAGE_CELLS * PAGE_CELLS)
        .map(|index| {
            let (q, first) = GROUND.child_block(index);
            let ([x, y], c) = (GROUND.in_page(first), &children[q].cells);
            let four = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]].map(|[x, y]| c[GROUND.cell_index(x, y) as usize]);
            let mut cell = 0 as GroundCell;
            for key in GROUND_CH_KEYS {
                let v = four.map(|f| channel_of(f, key));
                cell = with_channel(cell, key, majority(v, level as i32));
            }
            cell
        })
        .collect::<Vec<_>>();
    Ground { cells: out.into() }
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

/// Terrain reads the ring around the four children as well: a parent's
/// elevation pads two samples past its rim, which lie in its children's
/// neighbours (`fold_ele`).
impl Folds for Terrain {
    fn sources(p: LevelPage) -> Vec<LevelPage> {
        neighbourhood(p).to_vec()
    }

    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain_held(neighbourhood(p).map(held), p.level)
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
    let child = |q: usize| around[CHILDREN[q]].map(|t| &t.ground);
    let ground = fold_ground([child(0)?, child(1)?, child(2)?, child(3)?], level);
    let mut ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    Some(Terrain { ground, ele: ele.into() })
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
