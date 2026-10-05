// pxl-spec/space/zoom.rs

//! Every size is a zoom: the X lane is one world of 2^24 units, so a thing
//! the world over 2^z wide is at zoom z. A cell's, a chunk's and a tile's
//! size are each named by this one number; a bigger zoom is finer, and one
//! zoom finer is half as wide.

use super::pvf3::core::{FRAC_W, IVEC_X_W};
use super::pvf3::pvf3::{self, Pvf3};
use super::v3::Aabb;
use xpute_core::math::scalar::round;

pub type Zoom = u8;

/// A unit's: 1u is a z24 cell's side.
pub const UNIT_Z: Zoom = IVEC_X_W as Zoom;

/// The finest a point resolves: a fraction's step.
pub const FINEST_Z: Zoom = UNIT_Z + FRAC_W as Zoom;

/// log2 of `z`'s side in units: negative where finer than a unit.
pub const fn shamt(z: Zoom) -> i32 {
    UNIT_Z as i32 - z as i32
}

/// `z`'s side in units.
pub const fn units(z: Zoom) -> f64 {
    let s = shamt(z);
    if s >= 0 {
        (1u64 << s) as f64
    } else {
        1.0 / (1u64 << -s) as f64
    }
}

/// How many `inner` cells stand across one `outer`, which is no finer.
pub const fn count(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!()
    }
    1 << (inner - outer)
}

/// The zoom whose side is `units`, a power of two.
pub fn of_side(units: f64) -> Zoom {
    (UNIT_Z as i32 - round(libm::log2(units)) as i32) as Zoom
}

/// A floor on the signed lane.
pub const fn index(z: Zoom, units: i64) -> i64 {
    let s = shamt(z);
    if s >= 0 {
        units >> s
    } else {
        units << -s
    }
}

/// A floor on each signed lane.
pub fn index_of(z: Zoom, at: &Pvf3) -> [i64; 3] {
    let shift = FRAC_W as i32 + shamt(z);
    [pvf3::X(at) >> shift, pvf3::Y(at) >> shift, pvf3::Z(at) >> shift]
}

/// `index_of`'s inverse.
pub fn base_of(z: Zoom, index: [i64; 3]) -> Pvf3 {
    let shift = FRAC_W as i32 + shamt(z);
    pvf3::gather(index[0] << shift, index[1] << shift, index[2] << shift)
}

pub fn cube(z: Zoom, index: [i64; 3]) -> Aabb {
    let side = units(z);
    let min = index.map(|c| c as f64 * side);
    Aabb { min, max: min.map(|v| v + side) }
}

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
