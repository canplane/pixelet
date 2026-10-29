// pxl-kernel/domain/scene/decor/drawn.test.rs

use super::*;
use crate::domain::scene::decor::param::{DECOR_MODEL_CELLS, DECOR_SPRITE_CELLS};
use crate::domain::view::frame::pose::view_resize;
use crate::sys;
use pxl_spec::space::camera::grid::CELL;

/// Decor's tiers are angles: the model's and the sprite's edges are their
/// shares of the view's cell (`CELL`), whatever the window's height.
#[test]
fn decor_edges_cell_shares() {
    let _g = sys::serial();
    let edges = (DECOR_MODEL_CELLS * CELL, DECOR_SPRITE_CELLS * CELL);
    for h in [600.0, 844.0, 1080.0, 1440.0, 2160.0] {
        view_resize(h * 16.0 / 9.0, h);
        let (model, sprite) = (decor_model_angle(), decor_sprite_angle());
        assert!((model - edges.0).abs() < 1e-15, "{h}: {model}");
        assert!((sprite - edges.1).abs() < 1e-15, "{h}: {sprite}");
    }
}

/// What the ground grows is grown at the leaves' window alone, no coarser
/// one, so it is one set wherever it is drawn: every kind's zooms are the
/// leaf cell's bit or none.
#[test]
fn decor_leaf_window_only() {
    let leaf = 1 << (LEAF_CELL - FIRST_CELL);
    // From a tuft to a ship: what a coarser window's cells would show too.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in shown_by(|_| extent).iter().enumerate() {
            assert_eq!(bits, leaf, "{:?} {extent} u wide: {bits:#b}", DECORS[k].id);
        }
    }
}
