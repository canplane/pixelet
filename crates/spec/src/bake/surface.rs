// pxl-spec/bake/surface.rs

//! The surface a tile's elevation draws and its bound over any piece of the
//! tile. The interpolant lives here because the index lane's bound is true
//! only of the function that draws it.

use pxl_portable::ground::{held, window};

use super::constant::{ele_index, EXTENT_CELL};
use super::separation::Separation;
use super::tile::terrain::GroundId;
use crate::block::cube::Ground;
use crate::space::layer::lattice::{EXTENT, GROUND};

/// The lowest and highest `bicubic` gives in the texel box, ends included:
/// the samples of every window it reads there.
pub fn bound(tile_ele: &[i16], [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> [i32; 2] {
    let reads = |t0: f64, t1: f64| window(t0 as f32).0..=window(t1 as f32).0 + 3;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for y in reads(y0, y1) {
        for x in reads(x0, x1) {
            let e = i32::from(tile_ele[ele_index(held(x), held(y))]);
            (lo, hi) = (lo.min(e), hi.max(e));
        }
    }
    [lo, hi]
}

/// Per `EXTENT` cell, the lowest and highest of the surface where a cell of
/// it has one, any bridge top and any tunnel floor; where nothing stands,
/// lowest over highest, so a node over it holds nothing (a void cell has no
/// surface, whatever height is stored under it). Saturates at i16, about
/// 78 km.
pub fn extent_of(tile_ele: &[i16], cells: &[GroundId], separation: &Separation) -> Vec<[i16; 2]> {
    let count = crate::space::coord::zoom::count(GROUND.cell(), EXTENT_CELL);
    let n = f64::from(count);
    let side = EXTENT.tile_cells();
    (0..side * side)
        .map(|index| {
            let [cx, cy] = EXTENT.in_tile(index).map(|v| v * count);
            let ground = (cy..cy + count).any(|y| (cx..cx + count).any(|x| Ground::grounds(u32::from(cells[GROUND.cell_index(x, y) as usize]))));
            let [ex, ey] = [cx, cy].map(f64::from);
            let [lo, hi] = if ground { bound(tile_ele, [ex, ex + n], [ey, ey + n]) } else { [i32::MAX, i32::MIN] };
            let high = separation.highest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MIN, i32::from);
            let low = separation.lowest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MAX, i32::from);
            let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            [clamp(lo.min(low)), clamp(hi.max(high))]
        })
        .collect()
}
