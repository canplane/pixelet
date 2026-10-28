// pxl-kernel/domain/scene/marker/arrangement.test.rs

use super::*;
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::space::camera::grid::REFERENCE_VIEWPORT_H;

/// The width a place's name wraps to is one on every screen: the reference
/// view's `SPACING_CELLS` grid cells in CSS pixels, whatever the window's
/// height, as the shell's text it lays out is.
#[test]
fn a_places_width_is_one_on_every_screen() {
    let _g = crate::sys::serial();
    let at = |h: f64| {
        view_resize(h * 16.0 / 9.0, h);
        spacing_px()
    };
    let reference = at(REFERENCE_VIEWPORT_H);
    for h in [600.0, 844.0, 1440.0, 2160.0] {
        assert_eq!(at(h), reference, "{h}");
    }
}
