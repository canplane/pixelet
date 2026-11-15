// pxl-spec/space/coord/adapter/3857.rs

//! Lengths: `_u` the plane's units, `_merc` the Mercator plane's meter (EPSG:3857's
//! coordinates), `_m` a meter on the ground. u and merc are one scale apart
//! everywhere (`U_PER_MERC`); merc and m by the scale factor at a latitude
//! (`scale_factor`), so a length in u per ground meter is `u_per_m`. A
//! height's meters are the ground's own, the scale its leaf tile was baked
//! by (`height_u_per_m`).

use core::f64::consts::TAU;

use crate::space::coord::q3::{Q3, to_real, unpack};
use crate::space::coord::world;
use crate::space::coord::world::{ROUND, base_of};
use crate::space::coord::zoom::{LEAF_CELL_ZOOM, LEAF_ZOOM, Zoom, units};

use super::epsg4326::EARTH_RADIUS_M;

/// x and y in merc, z in ground meters.
pub type G3857 = [f64; 3];

/// The cell zoom a web map's `zoom` names, whose 256-pixel tiles are 2^8
/// of its cells across.
pub const fn cell_zoom_of(web_zoom: i32) -> Zoom {
    (web_zoom + 8) as Zoom
}

/// The plane's units in a Mercator meter: EPSG:3857 spans 2πR once around the world.
pub(crate) const U_PER_MERC: f64 = ROUND / (TAU * EARTH_RADIUS_M);

/// The Mercator scale factor at `y_u`: sec φ = cosh(y / R). A whole tile takes the value at
/// its base.
pub(crate) fn scale_factor(y_u: f64) -> f64 {
    libm::cosh(y_u * TAU / ROUND)
}

pub fn u_per_m(y_u: f64) -> f64 {
    U_PER_MERC * scale_factor(y_u)
}

pub fn merc_from_u(u: f64) -> f64 {
    u / U_PER_MERC
}

pub fn u_from_merc(merc: f64) -> f64 {
    merc * U_PER_MERC
}

/// Leaf cells a ground meter at `y_u`, for height as for breadth.
pub fn per_m(y_u: f64) -> f32 {
    (u_per_m(y_u) / units(LEAF_CELL_ZOOM)) as f32
}

/// `m` ground meters in leaf cells, rounded half to even, where `per_m` is the leaf cells a
/// ground meter at that place. Every length given in meters becomes cells this way, in the
/// bake and in the client alike.
pub fn cells_of_m(m: f32, per_m: f32) -> i32 {
    libm::rintf(m * per_m) as i32
}

/// The world's units a ground meter of height at `y_u`: the scale the bake made the ground's
/// heights by there, its leaf tile's south-west corner's (`per_m`, one value a tile), which
/// the clip levels read. Every height shown or read in meters is converted by it (`project`,
/// `unproject`).
pub fn height_u_per_m(y_u: f64) -> f64 {
    let row = libm::floor(y_u / units(LEAF_ZOOM)) as i32;
    let corner = to_real(i64::from(base_of(LEAF_ZOOM, [0, row, 0])[1]));
    f64::from(per_m(corner)) * units(LEAF_CELL_ZOOM)
}

pub fn project(x: f64, y: f64, z: f64) -> Q3 {
    let [x, y, _] = world::within([u_from_merc(x), u_from_merc(y), 0.0]);
    world::point(x, y, z * height_u_per_m(y))
}

pub fn unproject(q: &Q3) -> G3857 {
    let [x, y, z] = unpack(q);
    [merc_from_u(x), merc_from_u(y), z / height_u_per_m(y)]
}

pub type BBox3857 = [G3857; 2];

pub fn get_tile_bbox(zoom: Zoom, tile: [i32; 2]) -> BBox3857 {
    let span = units(zoom);
    let min = tile.map(|v| f64::from(v) * span);
    [
        [merc_from_u(min[0]), merc_from_u(min[1]), 0.0],
        [merc_from_u(min[0] + span), merc_from_u(min[1] + span), 0.0],
    ]
}

#[cfg(test)]
#[path = "3857.test.rs"]
mod test;
