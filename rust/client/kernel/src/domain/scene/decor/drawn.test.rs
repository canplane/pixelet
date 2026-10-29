// pxl-kernel/domain/scene/decor/drawn.test.rs

use super::*;
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::space::camera::grid::{px_per_unit, REFERENCE_FOV_DEG, REFERENCE_VIEWPORT_H};

/// Decor's tiers are angles: the model's and the sprite's edges are the same
/// whatever the window's height, and at the reference view they are the
/// pixels they were set by, about twelve and three CSS pixels.
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
    let px = |angle: f64| angle * px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG);
    assert!((px(reference.0) - 12.0).abs() < 0.1, "{}", px(reference.0));
    assert!((px(reference.1) - 3.0).abs() < 0.05, "{}", px(reference.1));
}

/// What the ground grows is grown at the leaves' window alone, no coarser
/// one, so it is one set wherever it is drawn: every kind's zooms are the
/// leaf cell's bit or none.
#[test]
fn decor_grows_at_the_leaves_window_alone() {
    let leaf = 1 << (LEAF_CELL - FIRST_CELL);
    // From a tuft to a ship: what a coarser window's cells would show too.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in shown_by(|_| extent).iter().enumerate() {
            assert_eq!(bits, leaf, "{:?} {extent} u wide: {bits:#b}", DECORS[k].id);
        }
    }
}
