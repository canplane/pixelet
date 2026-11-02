// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's (pxl-engine grid.rs, which holds its knobs).
//! The node a cell is drawn with, whole, spans at least one cell from any
//! direction; one cell of the grid is one thing drawn, and what lies inside
//! it is its layer's to fold.

use crate::space::coord::zoom::{units, Zoom};
use pxl_engine::grid;

// ---- the view's knobs, beside pxl-engine grid.rs's ----

/// The tangent of half the vertical field of view, the view's (pxl-kernel
/// domain/view/frame/pose.rs `ray_of`): its height spans a unit at unit
/// distance, a field of 53.13° (`fov`).
pub const HALF_TAN: f64 = 0.5;

// ---- derived from the knobs ----

/// The vertical field of view, in radians (`HALF_TAN`).
pub fn fov() -> f64 {
    2.0 * libm::atan(HALF_TAN)
}

/// The view's cells a radian (pxl-engine `grid::CELLS_A_RAD`).
pub const CELLS_A_RAD: f64 = grid::CELLS_A_RAD as f64;

/// How many times as far a clip level holds its zoom's cells as the view draws
/// them (pxl-engine `grid::CLIP_MARGIN`).
pub const CLIP_MARGIN: f64 = grid::CLIP_MARGIN as f64;

/// The grid the bands and clip levels are sized for, the view's
/// `CLIP_MARGIN` times finer (pxl-engine `grid::CLIP_CELLS_A_RAD`).
pub const CLIP_CELLS_A_RAD: f64 = grid::CLIP_CELLS_A_RAD as f64;

/// The view's camera grid cell, an angle in radians, 1/`CELLS_A_RAD`
/// (pxl-engine `grid::CELL`).
pub const CELL: f64 = 1.0 / CELLS_A_RAD;

/// Where ground starts being drawn with cells at `cell_zoom`, going out: where
/// its side spans `SPANS` of the view's cells (pxl-engine `grid`). A clip
/// level holds them out to `CLIP_MARGIN` times as far.
pub const fn split_dist(cell_zoom: Zoom) -> f64 {
    CELLS_A_RAD * units(cell_zoom) / grid::SPANS as f64
}

/// The camera grid's cell zoom at `r2`, a distance from the eye squared: the
/// widest cell whose split distance is within the distance, which is the
/// finest whose side spans one of the view's cells there, `r / CELLS_A_RAD`.
/// A node drawn whole, all its faces, is at least its side wide from any
/// direction, so the face a ray meets does not enter. Finer than the
/// ground's near the eye, held to what a zoom can name: pxl-engine's, the
/// one rule the CPU and a GPU's cast size by, of the one input, a square.
pub fn cell_zoom_at(r2: f64) -> Zoom {
    grid::cell_zoom_at(r2 as f32)
}

#[cfg(test)]
#[path = "grid.test.rs"]
mod test;
