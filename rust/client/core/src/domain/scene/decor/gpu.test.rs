// pxl-core/domain/scene/decor/gpu.test.rs

use super::*;
use crate::domain::scene::decor::param::{DECOR_IMPOSTOR_CELLS, DECOR_MODEL_CELLS};
use crate::domain::view::frame::pose::view_resize;
use crate::sys;
use pxl_spec::space::camera::grid::CELL;

/// The edges between decor's tiers are angles. The model's edge and the
/// impostor's edge stay fixed shares of the view's cell (`CELL`) whatever the
/// screen's height.
#[test]
fn decor_edges_cell_shares() {
    let _g = sys::serial();
    let edges = (DECOR_MODEL_CELLS * CELL, DECOR_IMPOSTOR_CELLS * CELL);
    for h in [600.0, 844.0, 1080.0, 1440.0, 2160.0] {
        view_resize(h * 16.0 / 9.0, h);
        let (model, impostor) = (decor_model_angle(), decor_impostor_angle());
        assert!((model - edges.0).abs() < 1e-15, "{h}: {model}");
        assert!((impostor - edges.1).abs() < 1e-15, "{h}: {impostor}");
    }
}

/// A kind is generated at the leaves' clip level and at each coarser one where
/// a variant stands as itself (`scatter::stands_at`). Its clip levels run
/// unbroken from the leaves' upward. None of them has cells of which the
/// kind's largest box fills less than an eighth, since a folded cube of that
/// box would keep nothing there.
#[test]
fn decor_clip_levels_by_size() {
    let leaf = 1 << (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM);
    // Extents from a tuft of grass to a ship.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in shown_by(|_| extent).iter().enumerate() {
            let (d, at) = (&DECORS[k], |cell_zoom: Zoom| {
                bits >> (cell_zoom - FIRST_CELL_ZOOM) & 1 == 1
            });
            if bits == 0 {
                continue;
            }
            assert_ne!(bits & leaf, 0, "{:?} {extent} u: {bits:#b}", d.id);
            let coarsest = (FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM).find(|&c| at(c)).unwrap();
            assert!(
                (coarsest..=LEAF_CELL_ZOOM).all(at),
                "{:?} {extent} u: {bits:#b}",
                d.id
            );
            let volume = (0..d.row.variants as usize)
                .map(|v| {
                    d.row.sizes[v]
                        .iter()
                        .map(|&s| units(UNIT_ZOOM) * f64::from(s))
                        .product::<f64>()
                })
                .fold(0.0, f64::max);
            assert!(
                coarsest == LEAF_CELL_ZOOM
                    || 8.0 * volume >= units(coarsest) * units(coarsest) * units(coarsest),
                "{:?}",
                d.id
            );
        }
    }
}
