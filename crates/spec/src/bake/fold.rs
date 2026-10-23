// pxl-spec/bake/fold.rs

//! One zoom coarser: a parent tile from the tiles under it. A cell holds the
//! four looks under it, each child cell's own (pxl-portable `ground::own`,
//! their majority, as Earth Engine's `MODE` pyramiding, so a feature
//! narrower than half a cell rounds away at the zoom that reads it).
//! Elevation is the box average of the samples, which preserves a column's
//! volume; a parent's surface is its own bicubic of its own samples.

use crate::space::layer::lattice::Cells;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use crate::space::layer::tile::{neighbor_index, CHILDREN};
use xpute_kit::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};
use super::separation::fold_separation;
use super::tile::terrain::{Ground, TerrainTile, ELE_LANE};
use super::tile::Occupancy;
use crate::space::coord::zoom::Zoom;
use pxl_portable::ground::own;

/// The four looks of each parent cell: the own look of each child cell
/// under it, `dx + 2·dy`, as the zoom under the parent's reads it.
pub fn fold_ground(children: [&Ground; 4], zoom: Zoom) -> Ground {
    let under = GROUND.cell_zoom(zoom) + 1;
    let out = (0..TILE_CELLS * TILE_CELLS)
        .flat_map(|index| {
            let (q, block) = GROUND.child_cells(index);
            let cells = &children[q].cells;
            block.map(|i| own(u32::from_le_bytes(core::array::from_fn(|k| cells[4 * i as usize + k])), under) as u8)
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
        let grid = around[neighbor_index(qx as i32, qy as i32)]?;
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

/// A parent from the 4×4 tiles around its children; None if any of its own
/// four is missing. A missing ring tile only degrades the padding.
pub fn fold_terrain(around: [Option<&TerrainTile>; 16], zoom: Zoom) -> Option<TerrainTile> {
    let child = |q: usize| around[CHILDREN[q]];
    let children = [child(0)?, child(1)?, child(2)?, child(3)?];
    let mut ele = vec![0i16; ELE_LANE];
    fold_ele(around.map(|t| t.map(|t| &*t.ele)), &mut ele);
    Some(TerrainTile::of(
        ele,
        fold_ground(children.map(|t| &t.ground), zoom),
        fold_separation(around.map(|t| t.map(|t| &t.separation)), zoom),
        Occupancy::of_children(children.map(|t| t.occupancy)),
    ))
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
