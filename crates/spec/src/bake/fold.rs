// pxl-spec/bake/fold.rs

//! One zoom coarser: a parent tile from the tiles under it, run by both the
//! bake and the client so a deterministic rule agrees between them. Ground
//! takes the 2×2 majority of the zoom below (as Earth Engine's `MODE` pyramiding),
//! so a feature narrower than half a cell rounds away. Elevation is the box
//! average, which preserves a column's volume. The index lane is carried up
//! from the children's extents and widened by the parent's own surface.

use crate::space::layer::lattice::{EXTENT, GROUND, TILE_CELLS};
use crate::space::layer::tile::{neighbor_index, CHILDREN};
use xpute_kit::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};
use super::separation::fold_separation;
use super::surface::extent_of;
use super::tile::terrain::{join, Ground, Index, TerrainTile};
use super::tile::Occupancy;
use crate::space::coord::zoom::Zoom;
use pxl_portable::ground::majority;

/// A cell is one look, so the majority is the look's.
pub fn fold_ground(children: [&Ground; 4], zoom: Zoom) -> Ground {
    let out = (0..TILE_CELLS * TILE_CELLS)
        .map(|index| {
            let (q, block) = GROUND.child_cells(index);
            majority(block.map(|i| children[q].cells[i as usize]), GROUND.cell_zoom(zoom))
        })
        .collect::<Vec<_>>();
    Ground { cells: out.into() }
}

/// The elevation folded: `around[neighbor_index(qx, qy)]` is the padded grid of
/// the child at (qx, qy), qx, qy in −1..=2. A parent's padding reaches four
/// child samples past the block, so it is read from the neighbors' inner
/// samples. A ring tile not baked yet lends nothing: the block's own
/// children's padding stands in, its edge repeated past it, until the
/// neighbor is baked and this tile folded again.
pub fn fold_ele(around: [Option<&[i16]>; 16], out: &mut [i16]) {
    let inner = TILE_ELE_SPAN as i64;
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
/// 2×2 child cells under it. A missing child folds as the zero tile.
pub fn fold_index(children: [&Index; 4], own: Vec<[i16; 2]>) -> Index {
    let extent = own
        .into_iter()
        .enumerate()
        .map(|(index, [lo, hi])| {
            let (q, block) = EXTENT.child_cells(index as u32);
            let four = block.map(|i| children[q].extent()[i as usize]);
            four.into_iter().fold([lo, hi], join)
        })
        .collect::<Vec<_>>();
    Index::new(extent, Occupancy::of_children(children.map(|c| c.occupancy)))
}

/// A parent from the 4×4 tiles around its children; None if any of its own
/// four is missing. A missing ring tile only degrades the padding, so a zoom
/// out folds from what the client already holds.
pub fn fold_terrain(around: [Option<&TerrainTile>; 16], zoom: Zoom) -> Option<TerrainTile> {
    let child = |q: usize| around[CHILDREN[q]];
    let children = [child(0)?, child(1)?, child(2)?, child(3)?];
    let ground = fold_ground(children.map(|t| &t.ground), zoom);
    let mut ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    let separation = fold_separation(children.map(|t| &t.separation), zoom);
    let index = fold_index(children.map(|t| &t.index), extent_of(&ele, &ground.cells, &separation));
    Some(TerrainTile {
        ground,
        separation,
        ele: ele.into(),
        index,
    })
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
