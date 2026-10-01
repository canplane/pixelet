// pxl-spec/bake/way.rs

//! How far a way — a road, a path, a track — covers the cells along its
//! line, on the ground (pxl-geo-bake terrain/pipeline/ground.rs) and on a
//! deck (bake/deck.rs) alike, so a road is as wide on a bridge as it is on
//! the ground it comes off, and as wide along a diagonal as along an axis.
//!
//! A way covers a cell whose center lies within half its width of its line.
//! Half a cell's diagonal is the least that half is: narrower, a way along a
//! diagonal is cells that meet only at their corners — a way that cannot be
//! followed — where at that reach every cell of it shares a side with the
//! next.

use core::f64::consts::FRAC_1_SQRT_2;

/// How far from its line a way of `width` cells covers a cell's center.
pub fn reach(width: f64) -> f64 {
    (width * 0.5).max(FRAC_1_SQRT_2)
}

/// Whether a way of `width` cells covers the cell whose center lies
/// `distance` cells from its line.
pub fn covers(width: f64, distance: f64) -> bool {
    distance <= reach(width)
}
