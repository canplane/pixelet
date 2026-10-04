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
//! Engine is the tie's corner (`majority`).
//!
//! Elevation is the box average, and that is forced rather than chosen. A
//! column's volume is linear in its height, so the mean is what preserves it,
//! and the fraction the mean leaves is what a coarse cell's inset is cut from.
//! The value stays in units at every level: in the level's own cells the
//! rounding would eat that fraction.
//!
//! Decks are lines, and fold as lines (bake/deck.rs `fold_decks`): each
//! child's in the parent's cells, simplified to its cell, and one narrower
//! than half of it left out — the size rule the majority keeps for ground.
//!
//! The index lane is the one fold that is not a summary of the level below
//! alone. A mean lies inside what it averages but says nothing of how far
//! what it averages reaches, so a parent's extent is carried up from its
//! children's, which already hold everything under them, and widened by its
//! own surface, which the interpolation can lift past theirs.

use crate::space::pyramid::PAGE_CELLS;
use xpute_core::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
use super::deck::fold_decks;
use super::format::{Ground, Index, Occupancy, Terrain, GROUND_CH_KEYS};
use super::pack::{channel_of, with_channel, GroundCell};
use super::surface::extent_of;
use crate::space::pyramid::{neighbor_index, neighborhood, Folds, LevelPage, CHILDREN, EXTENT, GROUND};

/// The value filling more than half of a 2×2 (or 2×2×2) block, `v[dx + 2·dy +
/// 4·dz]`; otherwise the tie child's. The tie corner alternates by `level`,
/// since a fixed corner drifts the map half a coarse cell per level.
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

/// Each ground channel folded on its own, so a road never takes the land's
/// vote under it.
pub fn fold_ground(children: [&Ground; 4], level: u32) -> Ground {
    let out = (0..PAGE_CELLS * PAGE_CELLS)
        .map(|index| {
            let (q, block) = GROUND.child_block(index);
            let four = block.map(|i| children[q].cells[i as usize]);
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

/// The elevation folded: `around[neighbor_index(qx, qy)]` is the padded grid of
/// the child at (qx, qy), qx, qy in −1..=2. A parent's padding reaches four
/// child samples past the block, so it is read from the neighbors' inner
/// samples. A missing page is ocean (0); a missing ring page falls back to the
/// block's own padding, which changes only the outermost padding sample.
pub fn fold_ele(around: [Option<&[i16]>; 16], out: &mut [i16]) {
    let inner = PAGE_ELE_SPAN as i64;
    let (pad, padded) = (ELE_PAD as i64, ELE_PADDED_SPAN as usize);
    let read = |qx: i64, qy: i64, mx: i64, my: i64| -> Option<i64> {
        let grid = around[neighbor_index(qx, qy)]?;
        Some(grid[ele_index((mx - qx * inner + pad) as u32, (my - qy * inner + pad) as u32)] as i64)
    };
    let of_block = |mx: i64, my: i64| -> Option<i64> {
        let near = |m: i64| m.clamp(-pad, 2 * inner - 1 + pad);
        let (mx, my) = (near(mx), near(my));
        let q = |m: i64| m.div_euclid(inner).clamp(0, 1);
        read(q(mx), q(my), mx, my)
    };
    // One child sample by its index across the block (−2·pad .. 2·inner + 2·pad).
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
            out[ele_index(sx as u32, sy as u32)] = round(sum as f64 / 4.0) as i16;
        }
    }
}

/// A parent's index: each cell the union of its own surface's extent and the
/// 2×2 child cells under it. A missing child folds as the zero page.
pub fn fold_index(children: [&Index; 4], own: Vec<[i16; 2]>) -> Index {
    let extent = own
        .into_iter()
        .enumerate()
        .map(|(index, [lo, hi])| {
            let (q, block) = EXTENT.child_block(index as u32);
            let four = block.map(|i| children[q].extent()[i as usize]);
            four.iter().fold([lo, hi], |[lo, hi], &[clo, chi]| [lo.min(clo), hi.max(chi)])
        })
        .collect::<Vec<_>>();
    Index::new(extent, Occupancy::of_children(children.map(|c| c.occupancy)))
}

/// Terrain also reads the ring around the four children, for its elevation padding.
impl Folds for Terrain {
    fn sources(p: LevelPage) -> Vec<LevelPage> {
        neighborhood(p).to_vec()
    }

    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(neighborhood(p).map(held), p.level)
    }
}

/// A parent from the 4×4 pages around its children; None if any of its own
/// four is missing. A missing ring page only degrades the padding, so a zoom
/// out folds from what the client already holds.
fn fold_terrain(around: [Option<&Terrain>; 16], level: u32) -> Option<Terrain> {
    let child = |q: usize| around[CHILDREN[q]];
    let children = [child(0)?, child(1)?, child(2)?, child(3)?];
    let ground = fold_ground(children.map(|t| &t.ground), level);
    let mut ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    let decks = fold_decks(children.map(|t| &t.decks), level);
    let index = fold_index(children.map(|t| &t.index), extent_of(&ele, &ground.cells, &decks));
    Some(Terrain { ground, decks, ele: ele.into(), index })
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
