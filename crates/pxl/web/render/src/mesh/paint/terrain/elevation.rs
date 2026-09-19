// pxl-render/mesh/paint/terrain/elevation.rs

//! Coordinate prefix rules:
//! - c (chunk)  : chunk lattice index (e.g. cx, cy)
//! - d (delta)  : local loop delta (e.g. dx, dy)
//! - t (texel)  : continuous tile texel / pixel coordinates (e.g. tx, ty)
//! - s (source) : source-grid coordinates in the padded low-res array (e.g. sx, sy)
//!
//! Unit convention:
//! - [u]      : Canonical world unit (Standard coordinate space).
//! - [cell]   : Discrete raster unit. 1u = 2^-CELL_SHAMT cells.
//! - [texel]  : Continuous coordinate within the PAGE_CELL_SPAN grid [0, PAGE_CELL_SPAN].
//! - [sample] : Low-res elevation grid index (before padding).
//!
//! Scale relation:
//! - 1 [cell] = 1 [texel]
//! - Elevation stored in the i16 lane is in [cell] units.

use pxl_spec::bake::constant::{CHUNK_CELL_SPAN, ELE_PAD, ELE_PADDED_SPAN, PAGE_CELL_SPAN, PAGE_ELE_SPAN};
use pxl_spec::bake::format::Ground;
use pxl_spec::cell::cube_types;
use xpute_core::math::scalar::lerp;

use xpute_core::math::scalar::round;

const ELE_CELL_STEP: f64 = PAGE_CELL_SPAN as f64 / PAGE_ELE_SPAN as f64;

/// Bilinear elevation sample from the padded elevation tile raster.
///
/// Coordinate convention:
/// - (tx, ty) are continuous texel coordinates.
/// - source samples are center samples at: (i - pad + 0.5) * step
///
/// Therefore the source-grid coordinates are:
///   sx = tx / step + pad - 0.5
///   sy = ty / step + pad - 0.5
pub fn sample_ele_bilinear(page_ele: &[i16], tx: u32, ty: u32) -> i16 {
    let sx = (tx as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;
    let sy = (ty as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;
    let last = (ELE_PADDED_SPAN - 1) as f64;
    let sx0 = sx.floor().clamp(0.0, last);
    let sy0 = sy.floor().clamp(0.0, last);
    let sx1 = (sx0 + 1.0).clamp(0.0, last);
    let sy1 = (sy0 + 1.0).clamp(0.0, last);
    let sdx = sx - sx0;
    let sdy = sy - sy0;
    let span = ELE_PADDED_SPAN as usize;
    let at = |x: f64, y: f64| page_ele[y as usize * span + x as usize] as f64;
    let ab = lerp(at(sx0, sy0), at(sx1, sy0), sdx);
    let cd = lerp(at(sx0, sy1), at(sx1, sy1), sdx);
    round(lerp(ab, cd, sdy)) as i16
}

/// Bicubic elevation sampling from the padded source raster.
///
/// Notes:
/// - keeps the hot path allocation-free
/// - uses an unclamped fast path for the common interior case
/// - falls back to clamped sampling near tile boundaries
fn sample_ele_bicubic(page_ele: &[i16], tx: u32, ty: u32) -> i16 {
    let sx = (tx as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;
    let sy = (ty as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;

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

/// `sample_ele`: the bicubic sample.
pub fn sample_ele(page_ele: &[i16], tx: u32, ty: u32) -> i16 {
    sample_ele_bicubic(page_ele, tx, ty)
}

/// How much real ground elevation drops for water-look terrain (RIVER/LAKE/
/// WETLAND, and SEA — its own land type, not the WATER channel, see
/// is_water_family_at) — a real height change, not a cosmetic mesh-only
/// offset: it's applied here, at the one shared elevation query surface, so
/// every consumer (chunk mesh, placement/snap raycast, and
/// crates/pxl/web/kernel/src/layer/terrain/elevation.rs's own camera ground-snap, which reads a
/// page's raw ele directly) sees the same lowered height instead of only the
/// rendered mesh looking recessed while placement math, ground-snap, or a
/// cell marker still reasons about the un-recessed one.
pub const WATER_RECESS_CELLS: i16 = 1;

/// Mirrors the terrain paint's ground_material_at priority (LAND < WATER <
/// STRUCTURE < TRANSPORT, last non-VOID wins) enough to answer one question:
/// does this cell look like open water? SEA is checked separately from the
/// WATER channel: cube.rs models it as land.sea (a masked BARE-family id),
/// not one of WATER's own RIVER/LAKE/WETLAND types.
pub fn is_water_family_at(ground: &Ground, idx: u32) -> bool {
    let i = idx as usize;
    let void = cube_types::common::void;
    if ground.structure[i] != void {
        return false;
    }
    if ground.transport[i] != void {
        return false;
    }
    if ground.water[i] != void {
        return true;
    }
    ground.land[i] == cube_types::land::sea
}

/// Unpacks a dense (CHUNK_CELL_SPAN x CHUNK_CELL_SPAN) elevation chunk from
/// the coarse heightmap. Samples are taken at cell centers (0.5 offset) to
/// match DB-side resampling.
pub fn dense_ele(page_ele: &[i16], tx0: u32, ty0: u32, ground: Option<&Ground>) -> Vec<i16> {
    let mut out = vec![0; (CHUNK_CELL_SPAN * CHUNK_CELL_SPAN) as usize];
    dense_ele_region(page_ele, tx0, ty0, CHUNK_CELL_SPAN, ground, &mut out);
    out
}

/// Dense reconstruction for a renderer-owned square region. `ground`, when
/// given, must already be sliced/aligned to this same tx0/ty0/span region —
/// every water-family cell in it gets WATER_RECESS_CELLS subtracted from its
/// sampled height.
pub fn dense_ele_region<'a>(page_ele: &[i16], tx0: u32, ty0: u32, span: u32, ground: Option<&Ground>, out: &'a mut [i16]) -> &'a mut [i16] {
    let mut idx: u32 = 0;
    for dy in 0..span {
        let ty = ty0 + dy;
        for dx in 0..span {
            let tx = tx0 + dx;
            let mut ele = sample_ele(page_ele, tx, ty);
            if ground.is_some_and(|g| is_water_family_at(g, idx)) {
                ele -= WATER_RECESS_CELLS;
            }
            out[idx as usize] = ele;
            idx += 1;
        }
    }
    out
}

#[cfg(test)]
#[path = "elevation.test.rs"]
mod test;
