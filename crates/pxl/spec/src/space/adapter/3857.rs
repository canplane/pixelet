// pxl-spec/space/adapter/3857.rs

//! Units: the plane's u and the ground's metre, related by `scale_at`. The
//! DB's EPSG:3857 coordinates are crossed only through `to_db`/`from_db`.

use core::f64::consts::TAU;

use crate::space::indexer::from_idx;
use crate::space::pvf3::{projection, FVec, IVec, Pvf3, ROUND};
use crate::space::pyramid::LEAF_Z;
use crate::space::zoom::{units, Zoom};

use super::epsg4326::EARTH_RADIUS_M;

/// x and y in the DB's EPSG:3857 coordinates, z in ground metres.
pub type G3857 = [f64; 3];

/// A cell at `cell_zoom` as a web map's zoom, whose 256-pixel tiles are 2^8
/// of its cells across.
pub const fn web_zoom_of(cell_zoom: Zoom) -> i32 {
    cell_zoom as i32 - 8
}

/// The cell zoom a web map's `zoom` names: `web_zoom_of`'s inverse.
pub const fn cell_zoom_of(web_zoom: i32) -> Zoom {
    (web_zoom + 8) as Zoom
}

const U_PER_DB: f64 = ROUND / (TAU * EARTH_RADIUS_M); // EPSG:3857 spans 2πR

/// sec φ = cosh(y / R), at the tile's base so a tile shares one value.
pub fn scale_at(y_u: f64) -> f64 {
    let base = (y_u / units(LEAF_Z)).floor() * units(LEAF_Z);
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
pub fn get_tile_bbox(tile_idx: IVec) -> BBox3857 {
    let min = projection::unproject(from_idx(LEAF_Z, tile_idx), 0);
    let span = units(LEAF_Z);
    [[to_db(min[0]), to_db(min[1]), 0.0], [to_db(min[0] + span), to_db(min[1] + span), 0.0]]
}

#[cfg(test)]
#[path = "3857.test.rs"]
mod test;
