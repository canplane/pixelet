// pxl-kernel/domain/scene/decor/drawn.test.rs

use super::*;
use crate::domain::scene::decor::param::{DECOR_IMPOSTOR_CELLS, DECOR_MODEL_CELLS};
use crate::domain::view::frame::pose::view_resize;
use crate::sys;
use pxl_spec::space::camera::grid::CELL;

/// Decor's tiers are angles: the model's and the impostor's edges are their
/// shares of the view's cell (`CELL`), whatever the window's height.
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

/// A kind is generated at the leaves' window and at each coarser one where a
/// variant stands as itself (`scatter::stands_at`): its windows run from the
/// leaves' up, unbroken, never at one whose cell its largest box keeps no
/// eighth of, as a folded cube of it would keep none.
#[test]
fn decor_windows_by_size() {
    let leaf = 1 << (LEAF_CELL - FIRST_CELL);
    // From a tuft to a ship, as it shows.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in shown_by(|_| extent).iter().enumerate() {
            let (d, at) = (&DECORS[k], |cell: Zoom| {
                bits >> (cell - FIRST_CELL) & 1 == 1
            });
            if bits == 0 {
                continue;
            }
            assert_ne!(bits & leaf, 0, "{:?} {extent} u: {bits:#b}", d.id);
            let coarsest = (FIRST_CELL..=LEAF_CELL).find(|&c| at(c)).unwrap();
            assert!(
                (coarsest..=LEAF_CELL).all(at),
                "{:?} {extent} u: {bits:#b}",
                d.id
            );
            let volume = (0..d.row.variants as usize)
                .map(|v| {
                    d.row.sizes[v]
                        .iter()
                        .map(|&s| units(d.row.cell_zoom()) * f64::from(s))
                        .product::<f64>()
                })
                .fold(0.0, f64::max);
            assert!(
                coarsest == LEAF_CELL || 8.0 * volume >= units(coarsest).powi(3),
                "{:?}",
                d.id
            );
        }
    }
}
