// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's, and a chunk a cube of its cells.

use crate::space::coord::zoom::{units, Zoom, UNIT_ZOOM};

/// The camera grid, the one knob of resolution: a cell is an angle of
/// 1/`SPLIT` radians about the eye, so it is drawn from `SPLIT` of its sides
/// away out to twice that. Pixels are a screen's, derived from it
/// (`target_cell_px`). One cell of the grid is one thing drawn; what lies
/// inside it is its layer's to fold.
pub const SPLIT: f64 = 96.0;

/// The view `SPLIT` is stated at: 1080 px high and 50° across it.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;

/// Pixels a unit at unit distance spans, for a viewport `viewport_h` high
/// and `fov_deg` across it.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// The pixels a cell spans at its zoom's split distance, at the reference
/// view: about twelve.
pub fn target_cell_px() -> f64 {
    px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / SPLIT
}

/// Where ground starts being drawn with cells at `cell_zoom` at `Tau::FINEST`, going out.
pub const fn split_distance(cell_zoom: Zoom) -> f64 {
    SPLIT * units(cell_zoom)
}

/// The camera grid's cell zoom at `r` from the eye: the widest cell whose
/// split distance is within `r`. Distance only: with the surface angle in,
/// the skyline would be drawn coarsest. Finer than the ground's near the
/// eye, held to what a zoom can name.
pub fn cell_at(r: f64) -> Zoom {
    (UNIT_ZOOM as f64 - libm::floor(libm::log2(r / SPLIT))).clamp(0.0, Zoom::MAX as f64) as Zoom
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

    pub fn cell_at(self, r: f64) -> Zoom {
        cell_at(r * self.0)
    }
}

/// A chunk, what is built and drawn as one, is 2^CHUNK_STEPS of the camera
/// grid's cells a side: its zoom is its cells' less this.
pub const CHUNK_STEPS: Zoom = 5;

pub const CHUNK_CELLS: u32 = 1 << CHUNK_STEPS;

/// The least angle a chunk spans about the eye, at `Tau::FINEST`: its cells
/// are each `1 / SPLIT` radians where its zoom is drawn nearest and half that
/// at the band's far edge.
pub const CHUNK_ANGLE: f64 = CHUNK_CELLS as f64 / (2.0 * SPLIT);
