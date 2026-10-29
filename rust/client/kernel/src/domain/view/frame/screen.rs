// pxl-kernel/domain/view/frame/screen.rs

//! Where the view meets the screen's CSS pixels, and the only place it does:
//! the view's size, a point's place on it, and the units a pixel spans at a
//! distance. What the world shows is decided in angles, the camera grid's
//! cell (pxl-spec space/camera/grid.rs `CELL`); a pixel enters only for the
//! UI's layout and for input, and check-rules names the modules that may
//! read these (pxl-tool bin/check_rules.rs `SCREEN_USERS`).

use pxl_spec::space::camera::grid::HALF_TAN;

use crate::domain::view::frame::pose::view;

/// The view's width and height in CSS pixels.
pub(crate) fn size() -> [f64; 2] {
    let v = view();
    [v.width, v.height]
}

/// The CSS pixels a unit spans a unit from the eye: the view's height over
/// the height it spans there, twice its half-angle's tangent.
pub(crate) fn px_per_unit() -> f64 {
    view().height / (2.0 * HALF_TAN)
}

/// The units a CSS pixel spans at `dist` from the eye (`px_per_unit`): what a
/// drag at that distance moves, and what a thing there is worth on the screen.
pub(crate) fn units_per_px_at(dist: f64) -> f64 {
    2.0 * dist * HALF_TAN / view().height
}

/// `ndc` in [−1, 1], y up, as CSS pixels from the view's top left.
pub(crate) fn ndc_to_px([x, y]: [f64; 2]) -> [f64; 2] {
    let v = view();
    [(x * 0.5 + 0.5) * v.width, (-y * 0.5 + 0.5) * v.height]
}
