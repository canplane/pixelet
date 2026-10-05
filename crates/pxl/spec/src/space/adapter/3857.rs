// pxl-spec/space/adapter/3857.rs

//! Units: the plane's u and the ground's metre, related by `scale_at`. The
//! DB's EPSG:3857 coordinates are crossed only through `to_db`/`from_db`.

use core::f64::consts::TAU;

use crate::space::hierarchy::Scale;
use crate::space::indexer::{FROM_L3_IDX, L3_RANGE};
use crate::space::pvf3::core::IVEC_X_W;
use crate::space::pvf3::{projection, FVec, IVec, Pvf3, ROUND};

use super::epsg4326::EARTH_RADIUS_M;

/// x and y in the DB's EPSG:3857 coordinates, z in ground metres.
pub type G3857 = [f64; 3];

/// A cell `side` wide as a web map's zoom (256-pixel tiles): the X lane is one
/// world, so it spans 2^(IVEC_X_W − shamt) of them.
pub const fn zoom_of(side: Scale) -> i32 {
    IVEC_X_W as i32 - 8 - side.shamt()
}

/// The cell side a web map's `zoom` names: `zoom_of`'s inverse.
pub const fn side_at_zoom(zoom: i32) -> Scale {
    Scale::of(IVEC_X_W as i32 - 8 - zoom)
}

const U_PER_DB: f64 = ROUND / (TAU * EARTH_RADIUS_M); // EPSG:3857 spans 2πR

/// sec φ = cosh(y / R), at the page's base so a page shares one value.
pub fn scale_at(y_u: f64) -> f64 {
    let base = (y_u / L3_RANGE as f64).floor() * L3_RANGE as f64;
    libm::cosh(base * TAU / ROUND)
}

pub fn u_per_m(y_u: f64) -> f64 {
    U_PER_DB * scale_at(y_u)
}

pub fn to_db(u: f64) -> f64 {
    u / U_PER_DB
}

pub fn from_db(db: f64) -> f64 {
    db * U_PER_DB
}

pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    let [x, y, _] = projection::within([from_db(x), from_db(y), 0.0]);
    projection::project(x, y, z * u_per_m(y))
}

pub fn unproject(i: IVec, f: FVec) -> G3857 {
    let [x, y, z] = projection::unproject(i, f);
    [to_db(x), to_db(y), z / u_per_m(y)]
}

pub type BBox3857 = [G3857; 2];

/// The maximum is built in u: the exclusive +2^23u edge is not an IVec.
pub fn get_page_bbox(page_idx: IVec) -> BBox3857 {
    let min = projection::unproject(FROM_L3_IDX(page_idx), 0);
    let span = L3_RANGE as f64;
    [[to_db(min[0]), to_db(min[1]), 0.0], [to_db(min[0] + span), to_db(min[1] + span), 0.0]]
}

#[cfg(test)]
#[path = "3857.test.rs"]
mod test;
