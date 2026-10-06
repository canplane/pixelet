// pxl-spec/space/coord/world.rs

//! What the lanes mean: X goes once round the world, zoom 0's side; Y and Z
//! end where their lanes do. A position enters the world only here, held
//! inside it, so no lane ever wraps where the world does not.

use super::pvf3::core::{IVEC_Y_W, IVEC_Z_W, ONE};
use super::pvf3::pack::pack;
use super::pvf3::Pvf3;
use super::v3::{Aabb, V3};
use super::zoom::{units, Zoom};

/// Every point the world holds, in units. X is unbounded: the lane is one world round.
pub const REACH: Aabb = {
    let (y, z) = ((1u64 << (IVEC_Y_W - 1)) as f64, (1u64 << (IVEC_Z_W - 1)) as f64);
    Aabb {
        min: [f64::NEG_INFINITY, -y, -z],
        max: [f64::INFINITY, y, z],
    }
};

/// `v` with Y and Z held inside `REACH`, half-open.
pub fn within(v: V3) -> V3 {
    let top = |k: usize| REACH.max[k] - 1.0 / ONE as f64;
    [v[0], v[1].clamp(REACH.min[1], top(1)), v[2].clamp(REACH.min[2], top(2))]
}

/// The one door from doubles into a position: X goes round the world, Y and
/// Z stop at its ends.
pub fn point(x: f64, y: f64, z: f64) -> Pvf3 {
    pack(within([x, y, z]))
}

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
