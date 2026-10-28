// pxl-spec/space/camera/grid.rs

//! The camera grid, the one resolution: a cell is an angle about the eye,
//! its zoom the distance's (pxl-spec-core grid.rs, which holds its knobs).
//! The node a cell is drawn with, whole, spans at least one cell from any
//! direction; one cell of the grid is one thing drawn, and what lies inside
//! it is its layer's to fold.

use crate::space::coord::zoom::{units, Zoom};

// ---- the view's knobs, beside pxl-spec-core grid.rs's ----

/// The vertical field of view, in degrees: the view's (pxl-kernel
/// domain/view/frame/pose.rs `ray_of`) and the reference view's.
pub const FOV_DEG: f64 = 50.0;

/// Drawn pixels per CSS pixel at most: the finest ratio a surface is drawn at,
/// and where more pixels stop being seen. The CSS reference pixel subtends
/// 0.0213° (CSS Values and Units: 96 dpi at 28 inches), 46.9 a degree, and
/// the fovea resolves 94 a degree at most (Ashraf, Chapiro & Mantiuk,
/// "Resolution limit of the eye: how many pixels can we see?", Nature
/// Communications, 2025, doi:10.1038/s41467-025-64679-2): 94 / 46.9 is 2.
pub const RATIO_MAX: f64 = 2.0;

/// The reference view's height in CSS pixels, at `FOV_DEG`: where a size
/// set in CSS pixels is read.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;

// ---- derived from the knobs ----

/// The grid the bands and windows are sized for (pxl-spec-core
/// `grid::SPLIT`).
pub const SPLIT: f64 = pxl_spec_core::grid::SPLIT as f64;

/// The view's camera grid cell, an angle in radians: τ over `SPLIT` at the
/// view's τ, 1/`CELLS_A_RADIAN` (pxl-kernel domain/view/lens.rs `cell`).
pub const CELL: f64 = Tau::VIEW.scale() / SPLIT;

/// Pixels a unit at unit distance spans, for a viewport `viewport_h` high
/// and `fov_deg` across it.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// An angle about the eye in CSS pixels at the reference view.
pub fn reference_px(angle: f64) -> f64 {
    angle * px_per_unit(REFERENCE_VIEWPORT_H, FOV_DEG)
}

/// The view's cell in CSS pixels at the reference view: about twelve.
pub fn cell_css_px() -> f64 {
    reference_px(CELL)
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

    /// Every view's, whatever its screen: `SPLIT` scaled to the view's cell
    /// (`CELL`), which is pxl-spec-core `grid::WINDOW_MARGIN`. How many pixels
    /// a display draws it with is the drawing's, never what is observed.
    pub const VIEW: Tau = Tau(pxl_spec_core::grid::WINDOW_MARGIN as f64);

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
    /// A node drawn whole, all its faces, is at least its side wide from any
    /// direction, so the face a ray meets does not enter. Finer than the
    /// ground's near the eye, held to what a zoom can name: pxl-spec-core's, the
    /// one rule the CPU and a GPU's cast size by, of the one input, a square;
    /// the distance as τ scales it.
    pub fn cell_at(self, r2: f64) -> Zoom {
        pxl_spec_core::grid::cell_at((r2 * (self.0 * self.0)) as f32)
    }
}
