// pxl-spec/space/adapter/3857.rs

//! EPSG:3857 planar metres to and from canonical units in PVF3. `m` here is a
//! Mercator planar metre, true only at the equator. Mercator wrapping and
//! vertical scaling live only in this adapter.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use crate::space::indexer::{FROM_L3_IDX, L3_RANGE};
use crate::space::pvf3::core::{FRAC_W, IVEC_X_W, IVEC_Y_W};
use crate::space::pvf3::{projection, FVec, IVec, Pvf3};

use super::epsg4326;

pub type G3857 = [f64; 3];

/// The projection's width at the equator; a tile pyramid divides it by `256 · 2^z`.
pub const WORLD_WIDTH_3857: f64 = 40075016.68557849;
const HALF_W_3857: f64 = WORLD_WIDTH_3857 / 2.0;

/// The 24-bit X lane is one Mercator world, so overflow is crossing the antimeridian.
const U_TOTAL_X: f64 = (1u64 << IVEC_X_W) as f64;

pub const U_PER_M: f64 = U_TOTAL_X / WORLD_WIDTH_3857; // 1 [m] ≈ 0.419 [u]
pub const M_PER_U: f64 = WORLD_WIDTH_3857 / U_TOTAL_X; // 1 [u] ≈ 2.389 [m]

// Y clamps rather than wraps, half-open: +2^23u is the lane's negative
// endpoint. The metre bound only feeds `Z_SCALE`; `project` clamps exactly
// in u, since a round trip through metres lands on the wrong lattice point.
const Y_MIN_3857: f64 = -HALF_W_3857;
const Y_MAX_3857: f64 = HALF_W_3857;

// Powers of two, so exact in f64.
const Y_MIN_U: f64 = -((1u64 << (IVEC_Y_W - 1)) as f64);
const Y_MAX_U: f64 = (1u64 << (IVEC_Y_W - 1)) as f64 - 1.0 / (1u64 << FRAC_W) as f64;

/// Mercator's vertical scale is evaluated at the owning L3 page's base, so
/// terrain, objects and rendering share one constant per page and never drift.
const Z_SCALE_STRIDE_U: f64 = L3_RANGE as f64;

fn Z_SCALE_FINE(y_m: f64) -> f64 {
    let lat_rad = (epsg4326::Y_TO_LAT(y_m) * core::f64::consts::PI) / 180.0;
    1.0 / libm::cos(lat_rad)
}

pub fn Z_SCALE(y_m: f64) -> f64 {
    let y_u = y_m * U_PER_M;
    let y_base_u = (y_u / Z_SCALE_STRIDE_U).floor() * Z_SCALE_STRIDE_U;
    let y_base_m = y_base_u * M_PER_U;
    Z_SCALE_FINE(y_base_m)
}

pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    let y = Y_MIN_U.max(Y_MAX_U.min(from_db(y)));
    projection::project(wrap_x(from_db(x)), y, z * u_per_m(y))
}

pub fn unproject(i: IVec, f: FVec) -> G3857 {
    let [x, y, z] = projection::unproject(i, f);
    [to_db(wrap_x(x)), to_db(y), z / u_per_m(y)]
}

pub type BBox3857 = [G3857; 2];

pub fn get_page_bbox(page_idx: IVec) -> BBox3857 {
    let min = projection::unproject(FROM_L3_IDX(page_idx), 0);
    let span = L3_RANGE as f64;
    [[to_db(min[0]), to_db(min[1]), 0.0], [to_db(min[0] + span), to_db(min[1] + span), 0.0]]
}

#[cfg(test)]
#[path = "3857.test.rs"]
mod test;
