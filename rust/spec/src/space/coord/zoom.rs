// pxl-spec/space/coord/zoom.rs

//! Every size is a zoom: X goes once round the world in 2^24 units, so a thing
//! the world over 2^z wide is at zoom z. A cell's, a square's and a tile's
//! size are each named by this one number; a bigger zoom is finer, and one
//! zoom finer is half as wide.

use super::aabb::Aabb;
use super::q3::{FRAC_W, Q3};
use super::world;
use xpute_kit::math::scalar::round;

pub use pxl_spec_core::Zoom;

/// A unit's: 1u is a z24 cell's side.
pub use pxl_spec_core::UNIT_ZOOM;

/// The coarsest cube: a tile here is as wide as the world is tall, so the
/// pyramid is alike on every axis from here finer, and 3D begins. A tile of
/// it with the world's whole height over it is a **shard**, the unit the world's data
/// is ingested, baked and kept in; what is ingested of one is it and its
/// apron.
pub use pxl_spec_core::SHARD_ZOOM;

/// Where Z takes its first digit: the cubes under a shard, its 3D roots.
pub const CUBE_ZOOM: Zoom = SHARD_ZOOM + 1;

/// The finest a point resolves: a fraction's step.
pub use pxl_spec_core::FINEST_ZOOM;

/// log2 of `z`'s side in units: negative where finer than a unit. `space/`'s
/// alone: the lattice is shifted here and nowhere else.
pub(in crate::space) const fn shamt(zoom: Zoom) -> i32 {
    UNIT_ZOOM as i32 - zoom as i32
}

/// `z`'s side in units.
pub const fn units(zoom: Zoom) -> f64 {
    let s = shamt(zoom);
    if s >= 0 {
        (1u64 << s) as f64
    } else {
        1.0 / (1u64 << -s) as f64
    }
}

/// How many `inner` cells stand across one `outer`, which is no finer.
pub use pxl_spec_core::count;

/// The zoom whose side is `units`, a power of two.
pub fn of_side(units: f64) -> Zoom {
    (UNIT_ZOOM as i32 - round(libm::log2(units)) as i32) as Zoom
}

/// The zoom of the widest power-of-two side no wider than `units`, signed:
/// past the world it is below zero.
pub fn within(units: f64) -> i32 {
    UNIT_ZOOM as i32 - libm::floor(libm::log2(units)) as i32
}

/// A floor on a signed axis.
pub const fn index(zoom: Zoom, units: i64) -> i64 {
    let s = shamt(zoom);
    if s >= 0 {
        units >> s
    } else {
        units << -s
    }
}

/// The cell at `zoom` a position stands in, a floor on each axis.
pub fn index_of(zoom: Zoom, at: &Q3) -> [i32; 3] {
    let shift = FRAC_W as i32 + shamt(zoom);
    // Shifted as an i64, as wide as zoom 0's 32 bits; the index is no wider
    // than the position it is of.
    at.map(|v| (i64::from(v) >> shift) as i32)
}

/// `index_of`'s inverse, to the cell's base.
pub fn base_of(zoom: Zoom, index: [i32; 3]) -> Q3 {
    let shift = FRAC_W as i32 + shamt(zoom);
    let [x, y, z] = index.map(i64::from);
    world::at(x << shift, y << shift, z << shift)
}

pub fn cube(zoom: Zoom, index: [i32; 3]) -> Aabb {
    let side = units(zoom);
    let min = index.map(|c| f64::from(c) * side);
    Aabb {
        min,
        max: min.map(|v| v + side),
    }
}

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
