// pxl-spec/space/adapter/4326.rs

//! EPSG:4326 <-> EPSG:3857 Projection Utilities
//!
//! ROLE
//! ----
//! Pure mathematical projection between:
//! - EPSG:4326 (WGS84 longitude/latitude)
//! - EPSG:3857 (Web Mercator meters)
//!
//! NOTES
//! -----
//! - This layer performs no canonical-unit scaling by itself.
//! - Scaling into G64 / canonical `u` space is handled by the 3857 adapter layer.
//! - Rendering conventions and axis remapping
//!   belong to higher layers.
//!
//! The optional altitude of a G4326 is its third lane here, 0 when absent.

#![allow(non_snake_case)]

use xpute_core::kit::fdlibm;

use crate::space::pvf3::{FVec, IVec, PVF3};

use super::epsg3857::{self, G3857};

/// EPSG:4326 (WGS84) geodetic coordinates: lng, lat, alt (meters above sea
/// level).
pub type G4326 = [f64; 3];

/// Latitude limits for Web Mercator.
/// Beyond these bounds, Y diverges to infinity.
const LAT_MAX: f64 = 85.05112878;
const LAT_MIN: f64 = -85.05112878;

/// Earth radius used by EPSG:3857 (meters).
const EARTH_RADIUS: f64 = 6378137.0;

const DEG_TO_RAD: f64 = core::f64::consts::PI / 180.0;
const RAD_TO_DEG: f64 = 180.0 / core::f64::consts::PI;

/// Normalizes longitude into [-180, 180).
fn NORMALIZE_LNG(lng: f64) -> f64 {
    ((((lng + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

/// Clamps latitude into the valid Web Mercator domain.
fn CLAMP_LAT(lat: f64) -> f64 {
    LAT_MIN.max(LAT_MAX.min(lat))
}

/// Longitude (degrees) -> Web Mercator X (meters).
fn LNG_TO_X(lng: f64) -> f64 {
    NORMALIZE_LNG(lng) * DEG_TO_RAD * EARTH_RADIUS
}

/// Latitude (degrees) -> Web Mercator Y (meters).
fn LAT_TO_Y(lat: f64) -> f64 {
    let lat_rad = CLAMP_LAT(lat) * DEG_TO_RAD;
    fdlibm::log(fdlibm::tan(core::f64::consts::PI / 4.0 + lat_rad / 2.0)) * EARTH_RADIUS
}

/// Web Mercator X (meters) -> longitude (degrees).
fn X_TO_LNG(x: f64) -> f64 {
    NORMALIZE_LNG((x / EARTH_RADIUS) * RAD_TO_DEG)
}

/// Web Mercator Y (meters) -> latitude (degrees).
pub fn Y_TO_LAT(y: f64) -> f64 {
    let lat_rad = 2.0 * fdlibm::atan(fdlibm::exp(y / EARTH_RADIUS)) - core::f64::consts::PI / 2.0;
    lat_rad * RAD_TO_DEG
}

// -----------------------------------------------------------------------------
// Core Projection (Math Only)
// -----------------------------------------------------------------------------

/// Projects EPSG:4326 -> EPSG:3857.
///
/// - Longitude is normalized (cyclic).
/// - Latitude is clamped (finite Mercator domain).
/// - Altitude is passed through unchanged when present.
pub fn to_mercator<'a>(dst: &'a mut G3857, src: &G4326) -> &'a mut G3857 {
    dst[0] = LNG_TO_X(src[0]);
    dst[1] = LAT_TO_Y(src[1]);
    dst[2] = src[2];
    dst
}

/// Unprojects EPSG:3857 -> EPSG:4326.
///
/// - Longitude is normalized for canonical output.
/// - Altitude is passed through unchanged.
pub fn from_mercator<'a>(dst: &'a mut G4326, src: &G3857) -> &'a mut G4326 {
    dst[0] = X_TO_LNG(src[0]);
    dst[1] = Y_TO_LAT(src[1]);
    dst[2] = src[2];
    dst
}

/// EPSG:4326 -> G64
/// 4326 geodetic -> 3857 planar meters -> G64 stored in PVF3
pub fn project<'a>(dst: &'a mut PVF3, src: &G4326) -> &'a mut PVF3 {
    // 1. LonLat -> Meters (Store in static buffer)
    let mut reg: G3857 = [0.0; 3];
    to_mercator(&mut reg, src);
    // 2. Meters -> Lattice (Write to dst)
    epsg3857::project(dst, reg[0], reg[1], reg[2]);
    dst
}

/// G64 -> EPSG:4326
/// G64 stored in PVF3 -> 3857 planar meters -> 4326 geodetic
pub fn unproject(out: &mut G4326, i: IVec, f: FVec) -> &mut G4326 {
    // 1. Lattice -> Meters (Store in static buffer)
    let mut reg: G3857 = [0.0; 3];
    epsg3857::unproject(&mut reg, i, f);
    // 2. Meters -> LonLat (Write to out)
    from_mercator(out, &reg);
    out
}

// ---- Distance ----

/// Computes great-circle distance (meters) using the haversine formula.
///
/// - Ignores altitude.
/// - Measures ground distance only.
pub fn dist(a: &G4326, b: &G4326) -> f64 {
    let d_lng = (b[0] - a[0]) * DEG_TO_RAD;
    let d_lat = (b[1] - a[1]) * DEG_TO_RAD;

    let sa = fdlibm::sin(d_lat / 2.0);
    let sb = fdlibm::sin(d_lng / 2.0);

    let h = sa * sa + fdlibm::cos(a[1] * DEG_TO_RAD) * fdlibm::cos(b[1] * DEG_TO_RAD) * sb * sb;

    2.0 * EARTH_RADIUS * fdlibm::atan2(h.sqrt(), (1.0 - h).sqrt())
}

// ---- Bounding Box ----

pub type BBox4326 = [G4326; 2];

pub fn get_page_bbox(page_idx: IVec) -> BBox4326 {
    let [min_m, max_m] = epsg3857::get_page_bbox(page_idx);

    let mut min: G4326 = [0.0; 3];
    let mut max: G4326 = [0.0; 3];
    from_mercator(&mut min, &min_m);
    from_mercator(&mut max, &max_m);

    // from_mercator normally canonicalizes +180° to -180°. A bbox maximum is
    // an exclusive interval endpoint rather than a coordinate, so preserve
    // +180° for the final eastern page instead of inverting the min/max pair.
    if max_m[0] > min_m[0] && max[0] < min[0] {
        max[0] = 180.0;
    }

    [min, max]
}
