// pxl-spec/space/adapter/4326.rs

//! EPSG:4326 (WGS84 lng/lat) to and from the DB's EPSG:3857 coordinates.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use crate::space::pvf3::{FVec, IVec, Pvf3};

use super::epsg3857::{self, G3857};

/// lng, lat, alt (metres above sea level).
pub type G4326 = [f64; 3];

/// Web Mercator's latitude limit; Y diverges past it.
const LAT_MAX: f64 = 85.05112878;
const LAT_MIN: f64 = -85.05112878;

pub const EARTH_RADIUS_M: f64 = 6378137.0; // WGS84 a, EPSG:3857's sphere

const DEG_TO_RAD: f64 = core::f64::consts::PI / 180.0;
const RAD_TO_DEG: f64 = 180.0 / core::f64::consts::PI;

/// Into [-180, 180).
fn NORMALIZE_LNG(lng: f64) -> f64 {
    ((((lng + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

fn CLAMP_LAT(lat: f64) -> f64 {
    LAT_MIN.max(LAT_MAX.min(lat))
}

fn LNG_TO_X(lng: f64) -> f64 {
    NORMALIZE_LNG(lng) * DEG_TO_RAD * EARTH_RADIUS_M
}

fn LAT_TO_Y(lat: f64) -> f64 {
    let lat_rad = CLAMP_LAT(lat) * DEG_TO_RAD;
    libm::log(libm::tan(core::f64::consts::PI / 4.0 + lat_rad / 2.0)) * EARTH_RADIUS_M
}

fn X_TO_LNG(x: f64) -> f64 {
    NORMALIZE_LNG((x / EARTH_RADIUS_M) * RAD_TO_DEG)
}

pub fn Y_TO_LAT(y: f64) -> f64 {
    let lat_rad = 2.0 * libm::atan(libm::exp(y / EARTH_RADIUS_M)) - core::f64::consts::PI / 2.0;
    lat_rad * RAD_TO_DEG
}

/// Longitude wraps, latitude clamps, altitude passes through.
pub fn to_mercator<'a>(dst: &'a mut G3857, src: &G4326) -> &'a mut G3857 {
    dst[0] = LNG_TO_X(src[0]);
    dst[1] = LAT_TO_Y(src[1]);
    dst[2] = src[2];
    dst
}

pub fn from_mercator<'a>(dst: &'a mut G4326, src: &G3857) -> &'a mut G4326 {
    dst[0] = X_TO_LNG(src[0]);
    dst[1] = Y_TO_LAT(src[1]);
    dst[2] = src[2];
    dst
}

pub fn project<'a>(dst: &'a mut Pvf3, src: &G4326) -> &'a mut Pvf3 {
    let mut reg: G3857 = [0.0; 3];
    to_mercator(&mut reg, src);
    epsg3857::project(dst, reg[0], reg[1], reg[2]);
    dst
}

pub fn unproject(out: &mut G4326, i: IVec, f: FVec) -> &mut G4326 {
    let mut reg: G3857 = [0.0; 3];
    epsg3857::unproject(&mut reg, i, f);
    from_mercator(out, &reg);
    out
}

/// Great-circle ground distance in metres (haversine), ignoring altitude.
pub fn dist(a: &G4326, b: &G4326) -> f64 {
    let d_lng = (b[0] - a[0]) * DEG_TO_RAD;
    let d_lat = (b[1] - a[1]) * DEG_TO_RAD;

    let sa = libm::sin(d_lat / 2.0);
    let sb = libm::sin(d_lng / 2.0);

    let h = sa * sa + libm::cos(a[1] * DEG_TO_RAD) * libm::cos(b[1] * DEG_TO_RAD) * sb * sb;

    2.0 * EARTH_RADIUS_M * libm::atan2(h.sqrt(), (1.0 - h).sqrt())
}

pub type BBox4326 = [G4326; 2];

pub fn get_page_bbox(page_idx: IVec) -> BBox4326 {
    let [min_m, max_m] = epsg3857::get_page_bbox(page_idx);

    let mut min: G4326 = [0.0; 3];
    let mut max: G4326 = [0.0; 3];
    from_mercator(&mut min, &min_m);
    from_mercator(&mut max, &max_m);

    // A bbox maximum is an exclusive endpoint, so keep +180° rather than
    // letting `from_mercator` turn it into -180°.
    if max_m[0] > min_m[0] && max[0] < min[0] {
        max[0] = 180.0;
    }

    [min, max]
}
