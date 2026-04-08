// @pxl/spec/space/adapter/4326.ts
//
// EPSG:4326 (WGS84 lon/lat) <-> EPSG:3857 <-> G64 (PVF3) adapter.
//
// This layer performs no canonical-unit scaling; that is handled by epsg3857.

use crate::space::indexer::{from_l3_idx, l3_limit};
use crate::space::pvf3::{fvec::FVec, ivec::IVec, pvf3::Pvf3};

use super::epsg3857::{self, y_to_lat, G3857};

/// EPSG:4326 (WGS84) geodetic coordinates: [lng, lat, alt_meters].
pub type G4326 = [f64; 3];

const LAT_MAX: f64 = 85.05112878;
const LAT_MIN: f64 = -85.05112878;
const EARTH_RADIUS: f64 = 6378137.0;

// ---- Mercator helpers ----

fn normalize_lng(lng: f64) -> f64 {
    (lng + 180.0).rem_euclid(360.0) - 180.0
}

fn clamp_lat(lat: f64) -> f64 {
    lat.clamp(LAT_MIN, LAT_MAX)
}

fn lng_to_x(lng: f64) -> f64 {
    normalize_lng(lng).to_radians() * EARTH_RADIUS
}

fn lat_to_y(lat: f64) -> f64 {
    let lat_rad = clamp_lat(lat).to_radians();
    (std::f64::consts::FRAC_PI_4 + lat_rad / 2.0).tan().ln() * EARTH_RADIUS
}

fn x_to_lng(x: f64) -> f64 {
    normalize_lng((x / EARTH_RADIUS).to_degrees())
}

/// Web Mercator Y (meters) -> latitude (degrees). Re-exported from epsg3857.
pub use super::epsg3857::y_to_lat as y_to_lat_pub;

// ---- Core Projection ----

/// EPSG:4326 -> EPSG:3857. Longitude normalized, latitude clamped, altitude unchanged.
pub fn to_mercator(src: G4326) -> G3857 {
    [lng_to_x(src[0]), lat_to_y(src[1]), src[2]]
}

/// EPSG:3857 -> EPSG:4326. Longitude normalized, altitude unchanged.
pub fn from_mercator(src: G3857) -> G4326 {
    [x_to_lng(src[0]), y_to_lat(src[1]), src[2]]
}

/// EPSG:4326 -> G64 (PVF3 lattice).
pub fn project(src: G4326) -> Pvf3 {
    let m = to_mercator(src);
    epsg3857::project(m[0], m[1], m[2])
}

/// G64 (PVF3 lattice) -> EPSG:4326.
pub fn unproject(i: IVec, f: FVec) -> G4326 {
    let m = epsg3857::unproject(i, f);
    from_mercator(m)
}

// ---- Distance ----

/// Great-circle distance (meters) via haversine formula. Ignores altitude.
pub fn dist(a: G4326, b: G4326) -> f64 {
    let d_lng = (b[0] - a[0]).to_radians();
    let d_lat = (b[1] - a[1]).to_radians();

    let sa = (d_lat / 2.0).sin();
    let sb = (d_lng / 2.0).sin();
    let h = sa * sa + a[1].to_radians().cos() * b[1].to_radians().cos() * sb * sb;

    2.0 * EARTH_RADIUS * h.sqrt().atan2((1.0 - h).sqrt())
}

// ---- Bounding Box ----

// NOTE: assumes the page does not cross the ±180° longitude seam.
pub type BBox4326 = [G4326; 2]; // [min, max]

pub fn get_page_bbox(page_idx: IVec) -> BBox4326 {
    let base = from_l3_idx(page_idx);
    let limit = l3_limit(base);

    let p1 = unproject(base, 0);
    let p2 = unproject(limit, 0);

    [
        [p1[0].min(p2[0]), p1[1].min(p2[1]), p1[2].min(p2[2])],
        [p1[0].max(p2[0]), p1[1].max(p2[1]), p1[2].max(p2[2])],
    ]
}
