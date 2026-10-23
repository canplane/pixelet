// pxl-spec/space/coord/world.rs

//! What a position means: X goes once round the world, zoom 0's side; Y and
//! Z end where the world does. A position enters the world only here, held
//! inside it, so nothing goes round where the world does not.

use super::aabb::Aabb;
use super::q3::{from_real, FRAC_W, ONE, Q3};
use super::v3::V3;
use super::zoom::{cube, units, Zoom};

/// The world's extent an axis, in bits of a unit:
///
/// - X, 24: a unit is a z24 cell's side, so the round is 2^24 units
///   (`zoom.rs` `UNIT_ZOOM`), a web map's z16 pixel a z24 cell;
/// - Y, 24: the Mercator plane is a square;
/// - Z, 16: ±2^15 units of height, as the terrain stores a height, an `i16`
///   in ground cells, a ground cell a unit (`bake/constant.rs`).
pub const EXTENT: [u32; 3] = [pxl_spec_core::UNIT_ZOOM, pxl_spec_core::UNIT_ZOOM, pxl_spec_core::HEIGHT_BITS];

const _: () = assert!(
    EXTENT[0] + FRAC_W == i32::BITS && EXTENT[1] + FRAC_W == i32::BITS && EXTENT[2] + FRAC_W <= i32::BITS,
    "X and Y fill a position's i32, so X goes round as one does"
);

/// Every point the world holds, in units. X is unbounded: it goes round.
pub const REACH: Aabb = {
    let (y, z) = ((1u64 << (EXTENT[1] - 1)) as f64, (1u64 << (EXTENT[2] - 1)) as f64);
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

/// The one door into a position, from steps of 1/256 unit: X goes round
/// the world as an `i32` does, Y and Z stop at its ends.
pub fn at(x: i64, y: i64, z: i64) -> Q3 {
    let hold = |v: i64, k: usize| {
        let half = 1i64 << (EXTENT[k] - 1 + FRAC_W);
        v.clamp(-half, half - 1) as i32
    };
    [x as i32, hold(y, 1), hold(z, 2)]
}

/// `at` from units.
pub fn point(x: f64, y: f64, z: f64) -> Q3 {
    at(from_real(x), from_real(y), from_real(z))
}

/// A ray's origin from units, which may stand off the world, as an eye
/// over it does: X goes round as at `at`, Y and Z stop only where an `i32`
/// does. Y fills its `i32`, so it stops where the world does; Z's spare
/// bits hold what is over the world, and a ray from there passes it whole
/// on its way down, as it passes any empty node.
pub fn origin(x: f64, y: f64, z: f64) -> Q3 {
    let hold = |v: f64| from_real(v).clamp(i32::MIN.into(), i32::MAX.into()) as i32;
    [from_real(x) as i32, hold(y), hold(z)]
}

/// From `a` to `b`, in steps: X the short way round, as an `i32` wraps; Y
/// and Z as they are, wide, since Y does not go round.
pub fn between(a: &Q3, b: &Q3) -> [i64; 3] {
    [i64::from(b[0].wrapping_sub(a[0])), i64::from(b[1]) - i64::from(a[1]), i64::from(b[2]) - i64::from(a[2])]
}

/// One round of the world, in units: zoom 0's side.
pub const ROUND: f64 = units(0);

/// How many cells at `z` stand around the world, and as many from its south edge to its north.
pub const fn around(zoom: Zoom) -> i64 {
    1i64 << zoom
}

/// A cell at `z` named as pxl-spec-core's `wrap_index` names its X, which
/// every key goes through.
pub fn wrap_cell(zoom: Zoom, [x, y, h]: [i32; 3]) -> [i32; 3] {
    [pxl_spec_core::wrap_index(zoom, x), y, h]
}

/// The copy of `x` nearest `about`: where a stored X stands for an eye at `about`.
pub fn nearest(x: f64, about: f64) -> f64 {
    // Whole laps added to `x`, so a point already on the lap comes back unchanged.
    x - ROUND * libm::rint((x - about) / ROUND)
}

/// `b` moved round the world to the copy nearest `about`.
pub fn nearest_box(b: &Aabb, about: f64) -> Aabb {
    let x = (b.min[0] + b.max[0]) / 2.0;
    if !x.is_finite() {
        return *b;
    }
    b.moved([nearest(x, about) - x, 0.0, 0.0])
}

/// A node's X moved by whole rounds to the round nearest `about`; as it is
/// where `about` is not finite.
pub fn node_near(zoom: Zoom, at: [i32; 3], about: f64) -> [i32; 3] {
    if !about.is_finite() {
        return at;
    }
    let whole = cube(zoom, at);
    let x = (whole.min[0] + whole.max[0]) / 2.0;
    let rounds = libm::rint((nearest(x, about) - x) / ROUND) as i32;
    [at[0] + rounds * (1 << zoom), at[1], at[2]]
}
