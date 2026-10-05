// pxl-spec/bake/fold.rs

//! One zoom coarser: a parent page from the pages under it, run by both the
//! bake and the client so a deterministic rule agrees between them. Ground
//! takes the 2×2 majority of the zoom below (as Earth Engine's `MODE` pyramiding),
//! so a feature narrower than half a cell rounds away. Elevation is the box
//! average, which preserves a column's volume. The index lane is carried up
//! from the children's extents and widened by the parent's own surface.

use crate::space::pyramid::PAGE_CELLS;
use xpute_core::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
use super::deck::fold_decks;
use super::format::{Ground, Index, Occupancy, Terrain};
use super::surface::extent_of;
use crate::space::pyramid::{neighbor_index, neighborhood, Folds, Page, CHILDREN, EXTENT, GROUND};

/// The value filling more than half of a 2×2 (or 2×2×2) block, `v[dx + 2·dy +
/// 4·dz]`; otherwise the tie child's. The tie corner alternates with the
/// parent's zoom `z`, since a fixed corner drifts the map half a coarse cell a zoom.
pub fn majority<T: Copy + Eq, const N: usize>(v: [T; N], z: i32) -> T {
    let tie = v[if z.rem_euclid(2) == 1 { 0 } else { N - 1 }];
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

/// A cell is one look, so the majority is the look's.
pub fn fold_ground(children: [&Ground; 4], z: u32) -> Ground {
    let out = (0..PAGE_CELLS * PAGE_CELLS)
        .map(|index| {
            let (q, block) = GROUND.child_block(index);
            majority(block.map(|i| children[q].cells[i as usize]), z as i32)
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
    fn sources(p: Page) -> Vec<Page> {
        neighborhood(p).to_vec()
    }

    /// A child the page marks as no page is its own ground already: open
    /// water, or nothing at all under a void page.
    fn settles(&self, p: Page, child: Page) -> bool {
        p.children().iter().position(|&c| c == child).is_some_and(|slot| !self.index.occupancy.has_child(slot))
    }

    fn fold_page<'a>(p: Page, held: impl Fn(Page) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(neighborhood(p).map(held), p.z)
    }
}

/// A parent from the 4×4 pages around its children; None if any of its own
/// four is missing. A missing ring page only degrades the padding, so a zoom
/// out folds from what the client already holds.
fn fold_terrain(around: [Option<&Terrain>; 16], z: u32) -> Option<Terrain> {
    let child = |q: usize| around[CHILDREN[q]];
    let children = [child(0)?, child(1)?, child(2)?, child(3)?];
    let ground = fold_ground(children.map(|t| &t.ground), z);
    let mut ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    let decks = fold_decks(children.map(|t| &t.decks), z);
    let index = fold_index(children.map(|t| &t.index), extent_of(&ele, &decks));
    Some(Terrain {
        ground,
        decks,
        ele: ele.into(),
        index,
    })
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
