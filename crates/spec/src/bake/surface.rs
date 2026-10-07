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

/// Catmull-Rom over the 4 × 4 padded samples about a texel point, in cells,
/// held to the samples' range, so a bound over the surface is the samples' own.
pub fn bicubic(tile_ele: &[i16], tx: f64, ty: f64) -> i16 {
    let sx = sample_at(tx);
    let sy = sample_at(ty);

    let sx0 = sx.floor() as i32;
    let sy0 = sy.floor() as i32;

    let wx = catmull_rom(sx - sx0 as f64);
    let wy = catmull_rom(sy - sy0 as f64);

    // A row's four samples are adjacent, read as one; the range is kept in
    // i16 on the integer side, where it does not compete with the sum's floats.
    let span = ELE_PADDED_SPAN as i32;
    let px = |i: i32| (sx0 - 1 + i).clamp(0, span - 1) as u32;
    let (mut out, mut lo, mut hi) = (0.0, i16::MAX, i16::MIN);
    for j in 0..4 {
        let py = (sy0 - 1 + j).clamp(0, span - 1) as u32;
        let r: [i16; 4] = if sx0 >= 1 && sx0 <= span - 3 {
            let at = ele_index(px(0), py) as usize;
            tile_ele[at..at + 4].try_into().expect("four samples")
        } else {
            core::array::from_fn(|i| tile_ele[ele_index(px(i as i32), py) as usize])
        };
        lo = lo.min(r[0].min(r[1]).min(r[2].min(r[3])));
        hi = hi.max(r[0].max(r[1]).max(r[2].max(r[3])));
        out += (f64::from(r[0]) * wx[0] + f64::from(r[1]) * wx[1] + f64::from(r[2]) * wx[2] + f64::from(r[3]) * wx[3]) * wy[j as usize];
    }
    round(out.clamp(f64::from(lo), f64::from(hi))) as i16
}

/// The lowest and highest `bicubic` gives in the texel box, ends included:
/// the samples of every 4 × 4 window it reads there.
pub fn bound(tile_ele: &[i16], [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> [i32; 2] {
    let span = ELE_PADDED_SPAN as i32;
    let reads = |t0: f64, t1: f64| sample_at(t0).floor() as i32 - 1..=sample_at(t1).floor() as i32 + 2;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for y in reads(y0, y1) {
        for x in reads(x0, x1) {
            let e = tile_ele[ele_index(x.clamp(0, span - 1) as u32, y.clamp(0, span - 1) as u32)] as i32;
            (lo, hi) = (lo.min(e), hi.max(e));
        }
    }
    [lo, hi]
}

/// Per `EXTENT` cell, the lowest and highest of the surface, any deck top
/// and any bore floor. Saturates at i16, about 78 km.
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
