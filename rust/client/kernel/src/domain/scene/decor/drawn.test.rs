// pxl-kernel/domain/scene/decor/drawn.test.rs

use super::*;
use crate::domain::scene::decor::param::{DECOR_MODEL_CELLS, DECOR_SPRITE_CELLS};
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::space::camera::grid::CELL;

/// Decor's tiers are angles: the model's and the sprite's edges are their
/// shares of the view's cell (`CELL`), whatever the window's height.
#[test]
fn decor_s_edges_are_shares_of_the_cell_on_every_screen() {
    let _g = crate::sys::serial();
    let edges = (DECOR_MODEL_CELLS * CELL, DECOR_SPRITE_CELLS * CELL);
    for h in [600.0, 844.0, 1080.0, 1440.0, 2160.0] {
        view_resize(h * 16.0 / 9.0, h);
        let (model, sprite) = (decor_model_angle(), decor_sprite_angle());
        assert!((model - edges.0).abs() < 1e-15, "{h}: {model}");
        assert!((sprite - edges.1).abs() < 1e-15, "{h}: {sprite}");
    }
}
