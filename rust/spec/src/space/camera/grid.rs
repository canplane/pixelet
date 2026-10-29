// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's.

use crate::space::coord::zoom::{units, Zoom};

// ---- the view's knobs, beside pxl-spec-core grid.rs's ----

/// The tangent of half the vertical field of view, the view's (pxl-kernel
/// domain/view/frame/pose.rs `ray_of`): its height spans a unit at unit
/// distance, a field of 53.13° (`fov`).
pub const HALF_TAN: f64 = 0.5;

// ---- derived from the knobs ----

/// The vertical field of view, in radians (`HALF_TAN`).
pub fn fov() -> f64 {
    2.0 * libm::atan(HALF_TAN)
}

/// The view's cells a radian (pxl-spec-core `grid::CELLS_A_RADIAN`).
pub const CELLS_A_RADIAN: f64 = pxl_spec_core::grid::CELLS_A_RADIAN as f64;

/// How many times as far a window holds its zoom's cells as the view draws
/// them (pxl-spec-core `grid::WINDOW_MARGIN`).
pub const WINDOW_MARGIN: f64 = pxl_spec_core::grid::WINDOW_MARGIN as f64;

/// The grid the bands and windows are sized for, the view's
/// `WINDOW_MARGIN` times finer (pxl-spec-core `grid::SPLIT`).
pub const SPLIT: f64 = pxl_spec_core::grid::SPLIT as f64;

/// The view `SPLIT` is stated at: 1080 CSS px high, drawn at
/// `RATIO_MAX`, and 50° across it.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;

/// Pixels a unit at unit distance spans, for a viewport `viewport_h` high
/// and `fov_deg` across it.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Drawn pixels per CSS pixel at most (pxl-spec-core `grid::RATIO_MAX`).
pub const RATIO_MAX: f64 = pxl_spec_core::grid::RATIO_MAX as f64;

/// The drawn pixels a camera grid's cell spans, at the reference view: about
/// twelve, at every ratio. A block drawn for it spans that to twice that.
pub fn target_cell_px() -> f64 {
    px_per_unit(REFERENCE_VIEWPORT_H * RATIO_MAX, REFERENCE_FOV_DEG) / SPLIT
}

/// Where ground starts being drawn with cells at `cell_zoom` at `Tau::FINEST`,
/// going out: where its side spans two of the camera grid's cells.
pub const fn split_distance(cell_zoom: Zoom) -> f64 {
    SPLIT * units(cell_zoom) / 2.0
}

/// The criterion's one scale factor, at least 1: every zoom's split
/// distance is `split_distance` / τ. The reach is sized at 1, and a
/// coarser τ only brings bands nearer, so none breaks it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Tau(f64);

impl Tau {
    pub const FINEST: Tau = Tau(1.0);

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
