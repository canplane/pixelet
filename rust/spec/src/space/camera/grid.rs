// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle around the eye, and its zoom
//! follows from the distance (pxl-engine grid.rs holds the knobs). The node a cell is drawn
//! with, drawn whole, spans at least one cell from any direction; one cell of the grid is one
//! thing drawn, and what lies inside it is for its layer to fold.

use crate::space::coord::zoom::{units, Zoom};
use pxl_engine::grid;

// The view's knobs are pxl-engine grid.rs's, mirrored here in f64.

/// The tangent of half the vertical field of view (pxl-engine
/// `grid::HALF_TAN`), a field of 53.13° (`fov`).
pub const HALF_TAN: f64 = grid::HALF_TAN as f64;

// What follows is derived from the knobs.

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

/// Where, going outward, ground starts being drawn with cells at `cell_zoom`: where a cell's
/// side spans `SPANS` of the view's cells (pxl-engine `grid`). A clip level holds those cells
/// out to `CLIP_MARGIN` times as far.
pub const fn split_dist(cell_zoom: Zoom) -> f64 {
    CELLS_A_RAD * units(cell_zoom) / grid::SPANS as f64
}

/// The camera grid's cell zoom at `r2`, a squared distance from the eye: the widest cell
/// whose split distance lies within the distance, which is the finest whose side spans one of
/// the view's cells there, `r / CELLS_A_RAD`. A node drawn whole, all its faces, is at least
/// its side wide from any direction, so the face a ray meets does not enter. Near the eye it
/// is finer than the ground's, held to the zooms that exist. It is pxl-engine's rule, by which
/// the CPU and a GPU's cast size their cells from the same input, a square.
pub fn cell_zoom_at(r2: f64) -> Zoom {
    grid::cell_zoom_at(r2 as f32)
}

#[cfg(test)]
#[path = "grid.test.rs"]
mod test;
