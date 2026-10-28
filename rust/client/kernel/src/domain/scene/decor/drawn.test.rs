// pxl-kernel/domain/scene/decor/drawn.test.rs

use super::*;
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::space::camera::grid::{cell_css_px, reference_px, REFERENCE_VIEWPORT_H};

/// Decor's tiers are angles: the model's and the sprite's edges are the same
/// whatever the window's height, and at the reference view they are the
/// cells they were set by, in CSS pixels (`cell_css_px`).
#[test]
fn decor_s_edges_are_angles_the_reference_view_draws_as_set() {
    let _g = crate::sys::serial();
    let at = |h: f64| {
        view_resize(h * 16.0 / 9.0, h);
        (decor_model_angle(), decor_sprite_angle())
    };
    let reference = at(REFERENCE_VIEWPORT_H);
    for h in [600.0, 844.0, 1440.0, 2160.0] {
        assert_eq!(at(h), reference, "{h}");
    }
    let cell = cell_css_px();
    let model = crate::domain::scene::decor::param::DECOR_MODEL_CELLS * cell;
    let sprite = crate::domain::scene::decor::param::DECOR_SPRITE_CELLS * cell;
    assert!((reference_px(reference.0) - model).abs() < 1e-9, "{}", reference_px(reference.0));
    assert!((reference_px(reference.1) - sprite).abs() < 1e-9, "{}", reference_px(reference.1));
}
