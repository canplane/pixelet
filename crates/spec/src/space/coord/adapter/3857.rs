// pxl-spec/space/coord/adapter/3857.rs

//! Lengths: `_u` the plane's units, `_mer` the Mercator plane's meter (EPSG:3857's
//! coordinates), `_m` a meter on the ground. u and mer are one scale apart
//! everywhere (`U_PER_MER`); mer and m by the scale factor at a latitude
//! (`scale_factor`), so a length in u per ground meter is `u_per_m`.

use core::f64::consts::TAU;

use crate::space::coord::pvf3::{pack, FVec, IVec, Pvf3};
use crate::space::coord::world;
use crate::space::coord::world::ROUND;
use crate::space::coord::zoom::{units, Zoom};

use super::epsg4326::EARTH_RADIUS_M;

/// x and y in mer, z in ground meters.
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

/// EPSG:3857 spans 2πR once round.
pub const U_PER_MER: f64 = ROUND / (TAU * EARTH_RADIUS_M);

/// sec φ = cosh(y / R). One value for a whole tile is its base's.
pub fn scale_factor(y_u: f64) -> f64 {
    libm::cosh(y_u * TAU / ROUND)
}

pub fn u_per_m(y_u: f64) -> f64 {
    U_PER_MER * scale_factor(y_u)
}

pub fn mer_from_u(u: f64) -> f64 {
    u / U_PER_MER
}

pub fn u_from_mer(mer: f64) -> f64 {
    mer * U_PER_MER
}

pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    let [x, y, _] = world::within([u_from_mer(x), u_from_mer(y), 0.0]);
    world::point(x, y, z * u_per_m(y))
}

pub fn unproject(i: IVec, f: FVec) -> G3857 {
    let [x, y, z] = pack::unpack(i, f);
    [mer_from_u(x), mer_from_u(y), z / u_per_m(y)]
}

pub type BBox3857 = [G3857; 2];

/// The square of the tile at `zoom` whose index is `tile`.
pub fn get_tile_bbox(zoom: Zoom, tile: [i64; 2]) -> BBox3857 {
    let span = units(zoom);
    let min = tile.map(|v| v as f64 * span);
    [[mer_from_u(min[0]), mer_from_u(min[1]), 0.0], [mer_from_u(min[0] + span), mer_from_u(min[1] + span), 0.0]]
}

#[cfg(test)]
#[path = "3857.test.rs"]
mod test;
