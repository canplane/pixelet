// pxl-spec/bake/way.rs

//! How far a way covers the cells along its line, on the ground and on a deck
//! alike. A way covers a cell whose center is within half its width, and that
//! half is at least half a cell's diagonal so a diagonal way stays side-connected.

use core::f64::consts::FRAC_1_SQRT_2;

use xpute_kit::math::scalar::round;

/// Half a way's width, in whole cells.
pub fn half_cells(width: f64) -> i32 {
    round(width / 2.0) as i32
}

/// How far from its line a way of `width` cells covers a cell's center.
pub fn reach(width: f64) -> f64 {
    (width * 0.5).max(FRAC_1_SQRT_2)
}

pub fn covers(width: f64, distance: f64) -> bool {
    distance <= reach(width)
}
