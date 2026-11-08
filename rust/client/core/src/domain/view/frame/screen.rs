// pxl-core/domain/view/frame/screen.rs

//! The only place the view meets the screen's CSS pixels. What the world
//! shows is decided in angles, by the camera grid's cell (pxl-spec
//! space/camera/grid.rs `CELL`). Pixels matter only to the UI's layout and
//! to input, and check-rules lets only the modules it names read this one
//! (pxl-tool bin/check_rules.rs `SCREEN_USERS`).

use pxl_spec::space::camera::grid::HALF_TAN;

use crate::domain::view::frame::pose::view;

/// The view's width and height in CSS pixels.
pub(crate) fn size() -> [f64; 2] {
    let v = view();
    [v.width, v.height]
}

/// How many CSS pixels one world unit spans at one unit from the eye. At that
/// distance the view's height covers twice the tangent of its half-angle.
pub(crate) fn px_per_unit() -> f64 {
    view().height / (2.0 * HALF_TAN)
}

/// How many world units one CSS pixel spans at `dist` from the eye. A drag
/// of one pixel at that distance moves this far, and a thing there this
/// size covers one pixel on the screen.
pub(crate) fn units_per_px_at(dist: f64) -> f64 {
    dist / px_per_unit()
}

/// `ndc`, in [−1, 1] with y up, as CSS pixels from the view's top left.
pub(crate) fn ndc_to_px([x, y]: [f64; 2]) -> [f64; 2] {
    let v = view();
    [(x * 0.5 + 0.5) * v.width, (-y * 0.5 + 0.5) * v.height]
}
