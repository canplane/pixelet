// pxl-spec/bake/surface.rs

//! The surface a page's elevation draws and its bound over any piece of the
//! page. The interpolant lives here because the index lane's bound is true
//! only of the function that draws it.

use xpute_core::math::scalar::round;

use super::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
use super::ground::cell_recess;
use super::pack::GroundCell;
use crate::space::pyramid::{EXTENT, GROUND, PAGE_CELLS};

const CELL_STEP: f64 = PAGE_CELLS as f64 / PAGE_ELE_SPAN as f64;

/// The continuous sample a texel coordinate falls at: a cell's center is its index plus a half.
fn sample_at(t: f64) -> f64 {
    t / CELL_STEP + ELE_PAD as f64 - 0.5
}

/// Catmull-Rom over the 4 × 4 padded samples about a texel point, in cells.
pub fn bicubic(page_ele: &[i16], tx: f64, ty: f64) -> i16 {
    let sx = sample_at(tx);
    let sy = sample_at(ty);

    let sx0 = sx.floor() as i32;
    let sy0 = sy.floor() as i32;

    let fdx = sx - sx0 as f64;
    let fdy = sy - sy0 as f64;

    let x2 = fdx * fdx;
    let x3 = x2 * fdx;
    let wx = [-0.5 * x3 + x2 - 0.5 * fdx, 1.5 * x3 - 2.5 * x2 + 1.0, -1.5 * x3 + 2.0 * x2 + 0.5 * fdx, 0.5 * x3 - 0.5 * x2];

    let y2 = fdy * fdy;
    let y3 = y2 * fdy;
    let wy = [-0.5 * y3 + y2 - 0.5 * fdy, 1.5 * y3 - 2.5 * y2 + 1.0, -1.5 * y3 + 2.0 * y2 + 0.5 * fdy, 0.5 * y3 - 0.5 * y2];

    let span = ELE_PADDED_SPAN as i32;
    let e = |idx: i32| page_ele[idx as usize] as f64;

    if sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3 {
        let row = |y: i32| {
            let idx = y * span + sx0;
            e(idx - 1) * wx[0] + e(idx) * wx[1] + e(idx + 1) * wx[2] + e(idx + 2) * wx[3]
        };
        return round(row(sy0 - 1) * wy[0] + row(sy0) * wy[1] + row(sy0 + 1) * wy[2] + row(sy0 + 2) * wy[3]) as i16;
    }

    let mut out = 0.0;
    for (j, wyj) in wy.iter().enumerate() {
        let py = (sy0 - 1 + j as i32).clamp(0, span - 1);
        let mut row_out = 0.0;
        for (i, wxi) in wx.iter().enumerate() {
            let px = (sx0 - 1 + i as i32).clamp(0, span - 1);
            row_out += e(py * span + px) * wxi;
        }
        out += row_out * wyj;
    }
    round(out) as i16
}

/// The lowest and highest `bicubic` gives in the texel box, ends included.
/// Catmull-Rom weights dip below zero by at most 1/8 an axis, so the
/// interpolant overshoots a 4 × 4 window's range by at most 9/32 of it.
pub fn bound(page_ele: &[i16], [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> [i32; 2] {
    let span = ELE_PADDED_SPAN as i32;
    let starts = |t0: f64, t1: f64| sample_at(t0).floor() as i32..=sample_at(t1).floor() as i32;
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for wy in starts(y0, y1) {
        for wx in starts(x0, x1) {
            let (mut wlo, mut whi) = (i32::MAX, i32::MIN);
            for y in wy - 1..=wy + 2 {
                for x in wx - 1..=wx + 2 {
                    let e = page_ele[(y.clamp(0, span - 1) * span + x.clamp(0, span - 1)) as usize] as i32;
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
pub fn extent_of(page_ele: &[i16], decks: &Decks) -> Vec<[i16; 2]> {
    let n = GROUND.scale(0).count_in(EXTENT.scale(0)) as f64;
    let side = EXTENT.page_cells();
    (0..side * side)
        .map(|index| {
            let [ex, ey] = EXTENT.in_page(index).map(|v| v as f64 * n);
            let [lo, hi] = bound(page_ele, [ex, ex + n], [ey, ey + n]);
            let (mut rlo, mut rhi) = (i32::MAX, i32::MIN);
            let mut last = None;
            for i in GROUND.under(EXTENT, index) {
                let cell = cells[i as usize];
                if last == Some(cell) {
                    continue;
                }
                last = Some(cell);
                let r = cell_recess(cell) as i32;
                (rlo, rhi) = (rlo.min(r), rhi.max(r));
            }
            let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            [clamp(lo - rhi), clamp(hi - rlo)]
        })
        .collect()
}
