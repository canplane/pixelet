// pxl-spec/space/coord/adapter/4326.rs

//! EPSG:4326 (WGS84 lng/lat) to and from the DB's EPSG:3857 coordinates.

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use crate::space::coord::q3::Q3;
use crate::space::coord::world::REACH;
use crate::space::coord::world::ROUND;
use crate::space::coord::zoom::Zoom;

use super::epsg3857::{self, G3857};

/// lng, lat, alt (meters above sea level).
pub type G4326 = [f64; 3];

pub use crate::space::coord::earth::EARTH_RADIUS_M;

const DEG_TO_RAD: f64 = core::f64::consts::PI / 180.0;
const RAD_TO_DEG: f64 = 180.0 / core::f64::consts::PI;

/// Into [-180, 180).
fn NORMALIZE_LNG(lng: f64) -> f64 {
    ((((lng + 180.0) % 360.0) + 360.0) % 360.0) - 180.0
}

/// Into the Mercator square's latitudes: its Y edge is half a world from
/// the equator, as wide as X is around, so the edge is atan(sinh π); Y
/// diverges past it.
fn CLAMP_LAT(lat: f64) -> f64 {
    let max = libm::atan(libm::sinh(core::f64::consts::TAU * REACH.max[1] / ROUND)) * RAD_TO_DEG;
    lat.clamp(-max, max)
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

/// The geoid's height over the WGS84 ellipsoid at `lng`, `lat` (EGM2008,
/// `geoid.spec.rs`), bilinear: what a GPS height, the ellipsoid's, is over
/// sea level, the terrain's. One value for a whole tile is its base's, as
/// the Mercator scale is.
pub fn geoid_m(lng: f64, lat: f64) -> f64 {
    use crate::space::coord::geoid::{GEOID_M, GEOID_STEP_DEG};
    let cols = (360.0 / GEOID_STEP_DEG) as usize;
    let rows = GEOID_M.len() / cols;
    let (r, c) = (
        (90.0 - CLAMP_LAT(lat)) / GEOID_STEP_DEG,
        (NORMALIZE_LNG(lng) + 360.0) % 360.0 / GEOID_STEP_DEG,
    );
    let (r0, c0) = ((r as usize).min(rows - 2), c as usize);
    let (fr, fc) = (r - r0 as f64, c - c0 as f64);
    let at = |r: usize, c: usize| f64::from(GEOID_M[r * cols + c % cols]);
    let row = |r: usize| at(r, c0) * (1.0 - fc) + at(r, c0 + 1) * fc;
    row(r0) * (1.0 - fr) + row(r0 + 1) * fr
}

/// Longitude wraps, latitude clamps, altitude passes through.
pub fn to_mercator(src: &G4326) -> G3857 {
    [LNG_TO_X(src[0]), LAT_TO_Y(src[1]), src[2]]
}

pub fn from_mercator(src: &G3857) -> G4326 {
    [X_TO_LNG(src[0]), Y_TO_LAT(src[1]), src[2]]
}

pub fn project(src: &G4326) -> Q3 {
    let [x, y, z] = to_mercator(src);
    epsg3857::project(x, y, z)
}

pub fn unproject(q: &Q3) -> G4326 {
    from_mercator(&epsg3857::unproject(q))
}

/// Great-circle ground distance in meters (haversine), ignoring altitude.
pub fn dist(a: &G4326, b: &G4326) -> f64 {
    let d_lng = (b[0] - a[0]) * DEG_TO_RAD;
    let d_lat = (b[1] - a[1]) * DEG_TO_RAD;

    let sa = libm::sin(d_lat / 2.0);
    let sb = libm::sin(d_lng / 2.0);

    let h = sa * sa + libm::cos(a[1] * DEG_TO_RAD) * libm::cos(b[1] * DEG_TO_RAD) * sb * sb;

    2.0 * EARTH_RADIUS_M * libm::atan2(h.sqrt(), (1.0 - h).sqrt())
}

pub type BBox4326 = [G4326; 2];

pub fn get_tile_bbox(zoom: Zoom, tile: [i32; 2]) -> BBox4326 {
    let [min_merc, max_merc] = epsg3857::get_tile_bbox(zoom, tile);

    let min = from_mercator(&min_merc);
    let mut max = from_mercator(&max_merc);

    // A bbox maximum is an exclusive endpoint, so keep +180° rather than
    // letting `from_mercator` turn it into -180°.
    if max_merc[0] > min_merc[0] && max[0] < min[0] {
        max[0] = 180.0;
    }

    [min, max]
}
