// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's, and a chunk a cube of its cells.

use crate::space::coord::zoom::{units, Zoom};

/// The camera grid, the one knob of resolution: a cell is an angle of
/// 1/`SPLIT` radians about the eye, and the block it is drawn with, whole,
/// spans at least one cell from any direction, so a zoom's block is drawn
/// from half `SPLIT` of its sides away out to `SPLIT` of them. Pixels are a screen's, derived from it
/// (`target_cell_px`). One cell of the grid is one thing drawn; what lies
/// inside it is its layer's to fold (pxl-portable, which a GPU's cast sizes
/// its cells by too).
pub const SPLIT: f64 = pxl_portable::cast::SPLIT as f64;

/// The view `SPLIT` is stated at: 1080 CSS px high, drawn at
/// `RATIO_MAX`, and 50° across it.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;

/// Pixels a unit at unit distance spans, for a viewport `viewport_h` high
/// and `fov_deg` across it.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Drawn pixels per CSS pixel at most (pxl-portable `cast::RATIO_MAX`).
pub const RATIO_MAX: f64 = pxl_portable::cast::RATIO_MAX as f64;

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

/// The camera grid's cell zoom at `r2`, a distance from the eye squared:
/// the widest cell whose split distance is within the distance, which is
/// the finest whose side spans one of the grid's cells there, `r / SPLIT`.
/// A block drawn whole, all its faces, is at least its side wide from any
/// direction, so the face a ray meets does not enter. Finer than the
/// ground's near the eye, held to what a zoom can name: pxl-portable's, the
/// one rule the CPU and a GPU's cast size by, of the one input, a square.
pub fn cell_at(r2: f64) -> Zoom {
    pxl_portable::cast::cell_at(r2 as f32)
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

    /// At `r2`, a distance squared.
    pub fn cell_at(self, r2: f64) -> Zoom {
        cell_at(r2 * (self.0 * self.0))
    }
}

/// A chunk, what is built and drawn as one, is 2^CHUNK_STEPS of the camera
/// grid's cells a side: its zoom is its cells' less this.
pub use pxl_portable::CHUNK_STEPS;
