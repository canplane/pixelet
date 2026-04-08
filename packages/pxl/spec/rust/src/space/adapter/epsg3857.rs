// @pxl/spec/space/adapter/3857.ts
//
// EPSG:3857 domain adapter.
// Bridges between EPSG:3857 planar meters and G64 (PVF3 lattice).

use crate::space::indexer::{from_l3_idx, l3_limit, L3_RANGE};
use crate::space::pvf3::{
    core::IVEC_X_W,
    fvec::FVec,
    ivec::IVec,
    projection,
    pvf3::Pvf3,
};

/// EPSG:3857 absolute coordinates in planar meters: [x, y, z].
pub type G3857 = [f64; 3];

const WORLD_WIDTH_3857: f64 = 40075016.68557849;
const HALF_W_3857: f64 = WORLD_WIDTH_3857 / 2.0;

const U_TOTAL_X: f64 = (1u64 << (IVEC_X_W - 1)) as f64; // 2^23 = 8,388,608 (MSB is guard bit)
pub const U_PER_M: f64 = U_TOTAL_X / WORLD_WIDTH_3857; // 1 [m] ≈ 0.209 [u]
const M_PER_U: f64 = WORLD_WIDTH_3857 / U_TOTAL_X;     // 1 [u] ≈ 4.777 [m]

const Z_SCALE_STRIDE_U: f64 = L3_RANGE as f64; // 128.0

// ---- Mercator helpers ----

/// Web Mercator Y (meters) -> latitude (degrees).
///
/// Defined here (rather than epsg4326) to break the circular import:
/// epsg3857 needs this for Z_SCALE_FINE; epsg4326 imports epsg3857.
pub fn y_to_lat(y: f64) -> f64 {
    let lat_rad = 2.0 * (y / 6378137.0).exp().atan() - std::f64::consts::FRAC_PI_2;
    lat_rad.to_degrees()
}

fn z_scale_fine(y_m: f64) -> f64 {
    let lat_rad = y_to_lat(y_m).to_radians();
    1.0 / lat_rad.cos()
}

/// Page-constant Mercator vertical scale for a given EPSG:3857 Y (meters).
///
/// Snaps `y_m` to the owning L3 page base in canonical `u` before evaluating,
/// so all points within the same L3 page return the same scale.
pub fn z_scale(y_m: f64) -> f64 {
    let y_u = y_m * U_PER_M;
    let y_base_u = (y_u / Z_SCALE_STRIDE_U).floor() * Z_SCALE_STRIDE_U;
    let y_base_m = y_base_u * M_PER_U;
    z_scale_fine(y_base_m)
}

// ---- Projection ----

/// EPSG:3857 planar meters -> G64 (PVF3 lattice).
///
/// Pipeline:
/// 1. Wrap X into canonical Mercator range (antimeridian wrap)
/// 2. Apply Mercator vertical scaling to altitude
/// 3. Meters -> canonical lattice units
/// 4. Project into PVF3
pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    let x = (x + HALF_W_3857).rem_euclid(WORLD_WIDTH_3857) - HALF_W_3857;
    let z = z * z_scale(y);
    projection::project(x * U_PER_M, y * U_PER_M, z * U_PER_M)
}

/// G64 (PVF3 lattice) -> EPSG:3857 planar meters.
pub fn unproject(i: IVec, f: FVec) -> G3857 {
    let [ux, uy, uz] = projection::unproject(i, f);
    let x = ux * M_PER_U;
    let y = uy * M_PER_U;
    let z = uz * M_PER_U;
    let x = (x + HALF_W_3857).rem_euclid(WORLD_WIDTH_3857) - HALF_W_3857;
    let z = z / z_scale(y);
    [x, y, z]
}

// ---- Bounding Box ----

/// Axis-aligned bounding box in EPSG:3857 meters.
pub type BBox3857 = [G3857; 2]; // [min, max]

pub fn get_page_bbox(page_idx: IVec) -> BBox3857 {
    let base = from_l3_idx(page_idx);
    let limit = l3_limit(base);
    let min = unproject(base, 0);
    let max = unproject(limit, 0);
    [min, max]
}
