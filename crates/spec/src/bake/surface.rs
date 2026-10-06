// pxl-spec/bake/surface.rs

//! The surface a tile's elevation draws and its bound over any piece of the
//! tile. The interpolant lives here because the index lane's bound is true
//! only of the function that draws it.

use xpute_core::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, EXTENT_CELL, TILE_ELE_SPAN};
use super::deck::Decks;
use crate::space::layer::lattice::{EXTENT, GROUND, TILE_CELLS};

const CELL_STEP: f64 = TILE_CELLS as f64 / TILE_ELE_SPAN as f64;

/// The continuous sample a texel coordinate falls at: a cell's center is its index plus a half.
fn sample_at(t: f64) -> f64 {
    t / CELL_STEP + ELE_PAD as f64 - 0.5
}

/// Catmull-Rom's four weights at `t` past the second sample.
fn catmull_rom(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1.0, -1.5 * t3 + 2.0 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2]
}

/// Catmull-Rom over the 4 × 4 padded samples about a texel point, in cells.
pub fn bicubic(tile_ele: &[i16], tx: f64, ty: f64) -> i16 {
    let sx = sample_at(tx);
    let sy = sample_at(ty);

    let sx0 = sx.floor() as i32;
    let sy0 = sy.floor() as i32;

    let wx = catmull_rom(sx - sx0 as f64);
    let wy = catmull_rom(sy - sy0 as f64);

    let span = ELE_PADDED_SPAN as i32;
    let e = |idx: usize| tile_ele[idx] as f64;

    if sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3 {
        let row = |y: i32| {
            let at = |x: i32| e(ele_index(x as u32, y as u32));
            at(sx0 - 1) * wx[0] + at(sx0) * wx[1] + at(sx0 + 1) * wx[2] + at(sx0 + 2) * wx[3]
        };
        return round(row(sy0 - 1) * wy[0] + row(sy0) * wy[1] + row(sy0 + 1) * wy[2] + row(sy0 + 2) * wy[3]) as i16;
    }

    let mut out = 0.0;
    for (j, wyj) in wy.iter().enumerate() {
        let py = (sy0 - 1 + j as i32).clamp(0, span - 1);
        let mut row_out = 0.0;
        for (i, wxi) in wx.iter().enumerate() {
            let px = (sx0 - 1 + i as i32).clamp(0, span - 1);
            row_out += e(ele_index(px as u32, py as u32)) * wxi;
        }
        out += row_out * wyj;
    }
    round(out) as i16
}

/// The lowest and highest `bicubic` gives in the texel box, ends included.
/// Catmull-Rom weights dip below zero by at most 1/8 an axis, so the
/// interpolant overshoots a 4 × 4 window's range by at most 9/32 of it.
pub fn bound(tile_ele: &[i16], [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> [i32; 2] {
    let span = ELE_PADDED_SPAN as i32;
    let starts = |t0: f64, t1: f64| sample_at(t0).floor() as i32..=sample_at(t1).floor() as i32;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for wy in starts(y0, y1) {
        for wx in starts(x0, x1) {
            let (mut wlo, mut whi) = (i32::MAX, i32::MIN);
            for y in wy - 1..=wy + 2 {
                for x in wx - 1..=wx + 2 {
                    let e = tile_ele[ele_index(x.clamp(0, span - 1) as u32, y.clamp(0, span - 1) as u32)] as i32;
                    (wlo, whi) = (wlo.min(e), whi.max(e));
                }
            }
            // Rounded up, which a rounded height cannot pass.
            let past = ((9 * (whi - wlo)) as u32).div_ceil(32) as i32;
            (lo, hi) = (lo.min(wlo - past), hi.max(whi + past));
        }
    }
    [lo, hi]
}

/// Per `EXTENT` cell, the lowest and highest of the surface (overshoot
/// included), any deck top and any bore floor. Saturates at i16, about 78 km.
pub fn extent_of(tile_ele: &[i16], decks: &Decks) -> Vec<[i16; 2]> {
    let n = crate::space::coord::zoom::count(GROUND.cell(), EXTENT_CELL) as f64;
    let side = EXTENT.tile_cells();
    (0..side * side)
        .map(|index| {
            let [ex, ey] = EXTENT.in_tile(index).map(|v| v as f64 * n);
            let [lo, hi] = bound(tile_ele, [ex, ex + n], [ey, ey + n]);
            let deck = decks.highest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MIN, i32::from);
            let bore = decks.lowest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MAX, i32::from);
            let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            [clamp(lo.min(bore)), clamp(hi.max(deck))]
        })
        .collect()
}
