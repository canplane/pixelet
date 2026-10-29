// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's (pxl-spec-core grid.rs, which holds its knobs).
//! The node a cell is drawn with, whole, spans at least one cell from any
//! direction; one cell of the grid is one thing drawn, and what lies inside
//! it is its layer's to fold.

use crate::space::coord::zoom::{units, Zoom};
use pxl_spec_core::grid;

// ---- the view's knobs, beside pxl-spec-core grid.rs's ----

/// The vertical field of view, in degrees: the view's (pxl-kernel
/// domain/view/frame/pose.rs `ray_of`).
pub const FOV_DEG: f64 = 50.0;

/// Drawn pixels per CSS pixel at most: the finest ratio a surface is drawn at,
/// and where more pixels stop being seen. The CSS reference pixel subtends
/// 0.0213° (CSS Values and Units: 96 dpi at 28 inches), 46.9 a degree, and
/// the fovea resolves 94 a degree at most (Ashraf, Chapiro & Mantiuk,
/// "Resolution limit of the eye: how many pixels can we see?", Nature
/// Communications, 2025, doi:10.1038/s41467-025-64679-2): 94 / 46.9 is 2.
pub const RATIO_MAX: f64 = 2.0;

// ---- derived from the knobs ----

/// The view's cells a radian (pxl-spec-core `grid::CELLS_A_RADIAN`).
pub const CELLS_A_RAD: f64 = grid::CELLS_A_RAD as f64;

/// How many times as far a window holds its zoom's cells as the view draws
/// them (pxl-spec-core `grid::WINDOW_MARGIN`).
pub const WINDOW_MARGIN: f64 = grid::WINDOW_MARGIN as f64;

/// The grid the bands and windows are sized for, the view's
/// `WINDOW_MARGIN` times finer (pxl-spec-core `grid::SPLIT`).
pub const SPLIT: f64 = grid::SPLIT as f64;

/// The view's camera grid cell, an angle in radians, 1/`CELLS_A_RADIAN`
/// (pxl-spec-core `grid::CELL`).
pub const CELL: f64 = 1.0 / CELLS_A_RAD;

/// Where ground starts being drawn with cells at `cell_zoom`, going out: where
/// its side spans `SPANS` of the view's cells (pxl-spec-core `grid`). A window
/// holds them out to `WINDOW_MARGIN` times as far.
pub const fn split_dist(cell_zoom: Zoom) -> f64 {
    CELLS_A_RAD * units(cell_zoom) / grid::SPANS as f64
}

/// The camera grid's cell zoom at `r2`, a distance from the eye squared: the
/// widest cell whose split distance is within the distance, which is the
/// finest whose side spans one of the view's cells there, `r / CELLS_A_RADIAN`.
/// A node drawn whole, all its faces, is at least its side wide from any
/// direction, so the face a ray meets does not enter. Finer than the
/// ground's near the eye, held to what a zoom can name: pxl-spec-core's, the
/// one rule the CPU and a GPU's cast size by, of the one input, a square.
pub fn cell_at(r2: f64) -> Zoom {
    grid::cell_at(r2 as f32)
}
