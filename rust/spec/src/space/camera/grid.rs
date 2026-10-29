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
/// domain/view/frame/pose.rs `ray_of`). The angle whose half's tangent is
/// 1/2, so the view's height spans a unit at unit distance
/// (doc/experiment/20261028-the-cell-and-the-fov.md).
pub const FOV_DEG: f64 = 53.130_102_354_155_99;


// ---- derived from the knobs ----

/// The vertical field of view, in radians (`HALF_TAN`).
pub fn fov() -> f64 {
    2.0 * libm::atan(HALF_TAN)
}

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
pub const fn split_distance(cell_zoom: Zoom) -> f64 {
    CELLS_A_RADIAN * units(cell_zoom) / pxl_spec_core::grid::SPANS as f64
}

/// The criterion's one scale factor, at least 1: every zoom's split
/// distance is `split_distance` / τ. The reach is sized at 1, and a
/// coarser τ only brings bands nearer, so none breaks it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Tau(f64);

impl Tau {
    pub const FINEST: Tau = Tau(1.0);

    /// Every view's, whatever its screen: the cell a ratio of one asks of
    /// `SPLIT`, which is stated at `RATIO_MAX`, so about twelve CSS pixels at
    /// the reference view. How many pixels a display draws it with is the
    /// drawing's, never what is observed.
    pub const VIEW: Tau = Tau(RATIO_MAX);

    /// At least 1: a finer quality than the criterion breaks the reach.
    pub fn of(scale: f64) -> Tau {
        xpute_kit::ensure!(scale >= 1.0, EINVAL);
        Tau(scale)
    }

    pub const fn scale(self) -> f64 {
        self.0
    }

    pub const fn split_distance(self, cell_zoom: Zoom) -> f64 {
        split_distance(cell_zoom) / self.0
    }

    /// The camera grid's cell zoom at `r2`, a distance from the eye squared:
    /// the widest cell whose split distance is within the distance, which is
    /// the finest whose side spans one of the grid's cells there, `r / SPLIT`.
    /// A block drawn whole, all its faces, is at least its side wide from any
    /// direction, so the face a ray meets does not enter. Finer than the
    /// ground's near the eye, held to what a zoom can name: pxl-spec-core's, the
    /// one rule the CPU and a GPU's cast size by, of the one input, a square;
    /// the distance as τ scales it.
    pub fn cell_at(self, r2: f64) -> Zoom {
        pxl_spec_core::grid::cell_at((r2 * (self.0 * self.0)) as f32)
    }
}

#[cfg(test)]
#[path = "grid.test.rs"]
mod test;
