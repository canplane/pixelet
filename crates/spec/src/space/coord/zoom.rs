// pxl-spec/space/coord/zoom.rs

//! Every size is a zoom: the X lane is one world of 2^24 units, so a thing
//! the world over 2^z wide is at zoom z. A cell's, a chunk's and a tile's
//! size are each named by this one number; a bigger zoom is finer, and one
//! zoom finer is half as wide.

use super::aabb::Aabb;
use super::pvf3::core::{FRAC_W, IVEC_X_W, IVEC_Z_W};
use super::pvf3::ivec::{self, IVec};
use super::pvf3::pvf3::{self, Pvf3};
use xpute_core::math::scalar::round;

pub type Zoom = u8;

/// A unit's: 1u is a z24 cell's side.
pub const UNIT_ZOOM: Zoom = IVEC_X_W as Zoom;

/// The coarsest cube: a tile here is as wide as the Z lane is tall, so the
/// pyramid is alike on every axis from here finer, and 3D begins. A tile of
/// it with the whole Z lane over it is a **shard**, the unit the world's data
/// is ingested, baked and kept in; what is ingested of one is it and its
/// apron.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - IVEC_Z_W as Zoom;

/// Where Z takes its first digit: the cubes under a shard, its 3D roots.
pub const CUBE_ZOOM: Zoom = SHARD_ZOOM + 1;

/// The finest a point resolves: a fraction's step.
pub const FINEST_ZOOM: Zoom = UNIT_ZOOM + FRAC_W as Zoom;

/// log2 of `z`'s side in units: negative where finer than a unit.
pub const fn shamt(zoom: Zoom) -> i32 {
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
pub const fn count(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!("an outer zoom finer than the inner")
    }
    1 << (inner - outer)
}

/// The zoom whose side is `units`, a power of two.
pub fn of_side(units: f64) -> Zoom {
    (UNIT_ZOOM as i32 - round(libm::log2(units)) as i32) as Zoom
}

/// The zoom of the widest power-of-two side no wider than `units`, signed:
/// past the world it is below zero.
pub fn within(units: f64) -> i32 {
    UNIT_ZOOM as i32 - libm::floor(libm::log2(units)) as i32
}

/// A floor on the signed lane.
pub const fn index(zoom: Zoom, units: i64) -> i64 {
    let s = shamt(zoom);
    if s >= 0 {
        units >> s
    } else {
        units << -s
    }
}

/// A floor on each signed lane.
pub fn index_of(zoom: Zoom, at: &Pvf3) -> [i64; 3] {
    let shift = FRAC_W as i32 + shamt(zoom);
    [pvf3::X(at) >> shift, pvf3::Y(at) >> shift, pvf3::Z(at) >> shift]
}

/// `index_of`'s inverse.
pub fn base_of(zoom: Zoom, index: [i64; 3]) -> Pvf3 {
    let shift = FRAC_W as i32 + shamt(zoom);
    pvf3::gather(index[0] << shift, index[1] << shift, index[2] << shift)
}

pub fn cube(zoom: Zoom, index: [i64; 3]) -> Aabb {
    let side = units(zoom);
    let min = index.map(|c| c as f64 * side);
    Aabb { min, max: min.map(|v| v + side) }
}

/// A lattice address to its cell's index at `zoom`, a floor on each lane;
/// each lane shifts on its own (`ivec::sh`), since a whole-word shift would
/// leak bits across lanes, and a lane narrower than a zoom's side collapses there.
pub fn idx(zoom: Zoom, addr: IVec) -> IVec {
    ivec::sh(addr, -shamt(zoom))
}

/// `idx`'s inverse, to the cell's base.
pub fn from_idx(zoom: Zoom, idx: IVec) -> IVec {
    ivec::sh(idx, shamt(zoom))
}

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
