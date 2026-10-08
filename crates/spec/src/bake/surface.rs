// pxl-spec/bake/surface.rs

//! The surface a tile's elevation draws and its bound over any piece of the
//! tile. The interpolant lives here because the index lane's bound is true
//! only of the function that draws it.

use xpute_kit::math::scalar::round;

use super::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, EXTENT_CELL, TILE_ELE_SPAN};
use super::separation::Separation;
use crate::space::layer::lattice::{EXTENT, GROUND, TILE_CELLS};

const CELL_STEP: f64 = TILE_CELLS as f64 / TILE_ELE_SPAN as f64;

/// The first of the four samples the surface reads along an axis about a
/// texel coordinate, and how far past the second the coordinate falls; a
/// cell's center is its index plus a half.
fn window(t: f64) -> (i32, f64) {
    let s = t / CELL_STEP + ELE_PAD as f64 - 0.5;
    let f = s.floor();
    (f as i32 - 1, s - f)
}

/// A sample index on the padded grid: past its edge, the edge sample stands.
/// It picks which sample is read; `bicubic`'s clamp is of the value.
fn held(i: i32) -> u32 {
    i.clamp(0, ELE_PADDED_SPAN as i32 - 1) as u32
}

/// Catmull-Rom's four weights at `t` past the second sample.
fn catmull_rom(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [-0.5 * t3 + t2 - 0.5 * t, 1.5 * t3 - 2.5 * t2 + 1.0, -1.5 * t3 + 2.0 * t2 + 0.5 * t, 0.5 * t3 - 0.5 * t2]
}

/// Catmull-Rom over the 4 × 4 padded samples about a texel point, in cells,
/// held to the samples' range, so a bound over the surface is the samples' own.
pub fn bicubic(tile_ele: &[i16], tx: f64, ty: f64) -> i16 {
    let (x0, fx) = window(tx);
    let (y0, fy) = window(ty);
    let (wx, wy) = (catmull_rom(fx), catmull_rom(fy));

    // A row's four samples are adjacent, read as one; the range is kept in
    // i16 on the integer side, where it does not compete with the sum's floats.
    let whole = x0 >= 0 && x0 + 3 < ELE_PADDED_SPAN as i32;
    let (mut out, mut lo, mut hi) = (0.0, i16::MAX, i16::MIN);
    for j in 0..4 {
        let py = held(y0 + j);
        let r: [i16; 4] = if whole {
            let at = ele_index(x0 as u32, py);
            tile_ele[at..at + 4].try_into().expect("four samples")
        } else {
            core::array::from_fn(|i| tile_ele[ele_index(held(x0 + i as i32), py)])
        };
        lo = lo.min(r[0].min(r[1]).min(r[2].min(r[3])));
        hi = hi.max(r[0].max(r[1]).max(r[2].max(r[3])));
        out += (f64::from(r[0]) * wx[0] + f64::from(r[1]) * wx[1] + f64::from(r[2]) * wx[2] + f64::from(r[3]) * wx[3]) * wy[j as usize];
    }
    round(out.clamp(f64::from(lo), f64::from(hi))) as i16
}

/// The lowest and highest `bicubic` gives in the texel box, ends included:
/// the samples of every window it reads there.
pub fn bound(tile_ele: &[i16], [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> [i32; 2] {
    let reads = |t0: f64, t1: f64| window(t0).0..=window(t1).0 + 3;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for y in reads(y0, y1) {
        for x in reads(x0, x1) {
            let e = i32::from(tile_ele[ele_index(held(x), held(y))]);
            (lo, hi) = (lo.min(e), hi.max(e));
        }
    }
    [lo, hi]
}

/// Per `EXTENT` cell, the lowest and highest of the surface, any bridge top
/// and any tunnel floor. Saturates at i16, about 78 km.
pub fn extent_of(tile_ele: &[i16], separation: &Separation) -> Vec<[i16; 2]> {
    let n = crate::space::coord::zoom::count(GROUND.cell(), EXTENT_CELL) as f64;
    let side = EXTENT.tile_cells();
    (0..side * side)
        .map(|index| {
            let [ex, ey] = EXTENT.in_tile(index).map(|v| v as f64 * n);
            let [lo, hi] = bound(tile_ele, [ex, ex + n], [ey, ey + n]);
            let high = separation.highest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MIN, i32::from);
            let low = separation.lowest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MAX, i32::from);
            let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            [clamp(lo.min(low)), clamp(hi.max(high))]
        })
        .collect()
}
