// pxl-spec/space/zoom.rs

//! Every size is a zoom: the X lane is one world of 2^24 units, so a thing
//! the world over 2^z wide is at zoom z. A cell's, a chunk's and a tile's
//! size are each named by this one number; a bigger zoom is finer, and one
//! zoom finer is half as wide.

use super::pvf3::core::{FRAC_W, IVEC_X_W, IVEC_Z_W};
use super::pvf3::ivec::{self, IVec};
use super::pvf3::pvf3::{self, Pvf3};
use super::v3::Aabb;
use xpute_core::math::scalar::round;

pub type Zoom = u8;

/// A unit's: 1u is a z24 cell's side.
pub const UNIT_Z: Zoom = IVEC_X_W as Zoom;

/// The coarsest cube that holds the whole Z lane: 3D begins here, and the
/// world's data is ingested a tile of it, a shard, at a time.
pub const SHARD_Z: Zoom = UNIT_Z - IVEC_Z_W as Zoom;

/// The finest a point resolves: a fraction's step.
pub const FINEST_Z: Zoom = UNIT_Z + FRAC_W as Zoom;

/// log2 of `z`'s side in units: negative where finer than a unit.
pub const fn shamt(zoom: Zoom) -> i32 {
    UNIT_Z as i32 - zoom as i32
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
    (UNIT_Z as i32 - round(libm::log2(units)) as i32) as Zoom
}

/// The zoom of the widest power-of-two side no wider than `units`, signed:
/// past the world it is below zero.
pub fn within(units: f64) -> i32 {
    UNIT_Z as i32 - libm::floor(libm::log2(units)) as i32
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

// ---- the world round: zoom 0's cell, once around in X ----

/// One round of the world, in units: zoom 0's side.
pub const ROUND: f64 = units(0);

/// How many cells at `z` stand around the world, and as many from its south edge to its north.
pub const fn around(zoom: Zoom) -> i64 {
    1i64 << zoom
}

/// An X index of `z`'s cells as the lane holds it; every key goes through it.
pub fn wrap_index(zoom: Zoom, x: i64) -> i64 {
    let across = around(zoom);
    (x + across / 2).rem_euclid(across) - across / 2
}

/// A cell at `z` named as `wrap_index` names its X.
pub fn wrap_cell(zoom: Zoom, [x, y, h]: [i64; 3]) -> [i64; 3] {
    [wrap_index(zoom, x), y, h]
}

/// The copy of `x` nearest `about`: where a stored X stands for an eye at `about`.
pub fn nearest(x: f64, about: f64) -> f64 {
    // Whole laps added to `x`, so a point already on the lap comes back unchanged.
    x - ROUND * libm::floor((x - about) / ROUND + 0.5)
}

/// `b` moved round the world to the copy nearest `about`.
pub fn nearest_box(b: &Aabb, about: f64) -> Aabb {
    let x = (b.min[0] + b.max[0]) / 2.0;
    if !x.is_finite() {
        return *b;
    }
    b.moved([nearest(x, about) - x, 0.0, 0.0])
}

#[cfg(test)]
#[path = "zoom.test.rs"]
mod test;
