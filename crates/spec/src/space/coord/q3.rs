// pxl-spec/space/coord/q3.rs

//! A position: three `i32`, one an axis, each counting 1/256 of a unit, the
//! same bits on every machine. An axis is its extent's integer bits and the
//! fraction's together (`world.rs` `EXTENT`): X and Y fill an `i32`, Z
//! takes 24 bits of one. What an axis means, and that X goes round, is the
//! world's; a position is made only through its door (`world::at`).

use super::v3::V3;

pub type Q3 = [i32; 3];

/// The fraction's bits: a unit is 256 steps (pxl-portable, which a GPU's
/// cast reads a position by too).
pub use pxl_portable::cast::FRAC_W;
pub const ONE: i64 = 1 << FRAC_W;

/// Steps as units.
pub fn to_real(steps: i64) -> f64 {
    steps as f64 / ONE as f64
}

/// Units as steps, a floor, wide: the world's door narrows them.
pub fn from_real(units: f64) -> i64 {
    libm::floor(units * ONE as f64) as i64
}

/// A position in units.
pub fn unpack(q: &Q3) -> V3 {
    q.map(|v| to_real(i64::from(v)))
}
