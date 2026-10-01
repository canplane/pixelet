// pxl-spec/bake/surface.rs

//! The surface a page's elevation draws, and how high it can stand over any
//! piece of the page. The interpolant is here rather than beside the mesh
//! because the bound is a claim about it: a page's index lane (bake/format.rs
//! `Index`) says no ray passes under or over what is drawn, and that is true
//! only of the function that draws it.

use xpute_core::math::scalar::round;

use super::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN};
use super::deck::Decks;
use super::ground::cell_recess;
use super::pack::GroundCell;
use crate::space::pyramid::{EXTENT, GROUND, PAGE_CELLS};

/// Cells a sample stands for along an axis.
const CELL_STEP: f64 = PAGE_CELLS as f64 / PAGE_ELE_SPAN as f64;

/// The sample a texel coordinate falls at, continuous: a cell's center is its
/// index and a half, and a sample's is its own (bake/constant.rs's sampling
/// contract).
fn sample_at(t: f64) -> f64 {
    t / CELL_STEP + ELE_PAD as f64 - 0.5
}

/// The elevation at a point in texels, continuous, in cells: the Catmull-Rom
/// interpolation of the 4 × 4 samples about it in the padded raster. Signed,
/// because the raster reaches `ELE_PAD` samples past the page's own span.
///
/// Notes:
/// - keeps the hot path allocation-free
/// - uses an unclamped fast path for the common interior case
/// - falls back to clamped sampling near the raster's edge
pub fn bicubic(page_ele: &[i16], tx: f64, ty: f64) -> i16 {
    let sx = sample_at(tx);
    let sy = sample_at(ty);

    let sx0 = sx.floor() as i32;
    let sy0 = sy.floor() as i32;

    let fdx = sx - sx0 as f64;
    let fdy = sy - sy0 as f64;

    // precompute cubic interpolation weights.
    let x2 = fdx * fdx;
    let x3 = x2 * fdx;
    let wx = [-0.5 * x3 + x2 - 0.5 * fdx, 1.5 * x3 - 2.5 * x2 + 1.0, -1.5 * x3 + 2.0 * x2 + 0.5 * fdx, 0.5 * x3 - 0.5 * x2];

    let y2 = fdy * fdy;
    let y3 = y2 * fdy;
    let wy = [-0.5 * y3 + y2 - 0.5 * fdy, 1.5 * y3 - 2.5 * y2 + 1.0, -1.5 * y3 + 2.0 * y2 + 0.5 * fdy, 0.5 * y3 - 0.5 * y2];

    let span = ELE_PADDED_SPAN as i32;
    let e = |idx: i32| page_ele[idx as usize] as f64;

    // fast interior path with no clamp overhead.
    if sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3 {
        let row = |y: i32| {
            let idx = y * span + sx0;
            e(idx - 1) * wx[0] + e(idx) * wx[1] + e(idx + 1) * wx[2] + e(idx + 2) * wx[3]
        };
        return round(row(sy0 - 1) * wy[0] + row(sy0) * wy[1] + row(sy0 + 1) * wy[2] + row(sy0 + 2) * wy[3]) as i16;
    }

    // slow boundary path with clamped coordinates.
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

/// The lowest and highest `bicubic` gives anywhere in the box of texels
/// `[x0, x1] × [y0, y1]`, both ends included, in cells.
///
/// Every point is interpolated from one 4 × 4 window, and the weights of a
/// window are negative by at most an eighth along an axis; the positive part
/// of the two axes' products then sums to at most (9/8)² + (1/8)² = 41/32, so
/// an interpolated height lies within 9/32 of the window's span past either
/// end of it. Taken window by window, so a hill in one corner of the box
/// widens only the windows that read it.
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
            // Taken up to a whole number, which a rounded height cannot
            // pass on its way out from the true bound.
            let past = ((9 * (whi - wlo)) as u32).div_ceil(32) as i32;
            (lo, hi) = (lo.min(wlo - past), hi.max(whi + past));
        }
    }
    [lo, hi]
}

/// A page's own surface's extent: for each of the index lattice's cells
/// (`EXTENT`), the lowest and highest the ground drawn anywhere in it stands,
/// in cells — the interpolation's overshoot and the cells' recess included,
/// so a point anywhere over the cell, a cell's center or a finer cell read
/// from this page, lies inside it — the highest any deck over it is, and the
/// lowest a bore through it cuts.
/// Saturated at an i16's range, which is some 78 km at the equator and is
/// never met.
pub fn extent_of(page_ele: &[i16], cells: &[GroundCell], decks: &Decks) -> Vec<[i16; 2]> {
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
            let deck = decks.highest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MIN, i32::from);
            let bore = decks.lowest_in([ex, ex + n], [ey, ey + n]).map_or(i32::MAX, i32::from);
            let clamp = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            [clamp((lo - rhi).min(bore)), clamp((hi - rlo).max(deck))]
        })
        .collect()
}
