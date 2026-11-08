// pxl-core/domain/view/lens.test.rs

use super::*;
use crate::sys::serial;

/// The clocks as they stepped before they were one (the sky's and the
/// clouds' in `EnvironmentDraw::observe`): `at` and what it shows, taken at
/// `t` where `t` reached the next step from `at` or went back before it.
fn stepped_as_written(at: &mut Option<f64>, t: f64, next: impl Fn(f64) -> f64) -> (f64, f64) {
    if at.is_none_or(|a| t >= next(a) || t < a) {
        *at = Some(t);
    }
    let a = at.unwrap_or(t);
    (a, next(a))
}

#[test]
fn stepped_clock_steps() {
    // A wall clock's readings, on and with a step back, and a next step
    // fixed, growing with the reading, and never.
    let readings = [
        1_000.0, 1_016.7, 1_033.3, 1_050.0, 1_066.6, 1_100.0, 1_040.0, 1_041.0, 1_200.0, 1_200.0,
    ];
    let nexts: [fn(f64) -> f64; 3] = [|a| a + 50.0, |a| a + a / 40.0, |a| a + f64::INFINITY];
    for next in nexts {
        let (mut stepped, mut before) = (Stepped::default(), None);
        for t in readings {
            let (shown, due, _) = stepped.at(t, next);
            let (was_shown, was_due) = stepped_as_written(&mut before, t, next);
            assert_eq!(
                [shown.to_bits(), due.to_bits()],
                [was_shown.to_bits(), was_due.to_bits()],
                "at {t}"
            );
        }
    }
}

#[test]
fn stepped_clock_reports() {
    let mut clock = Stepped::default();
    assert_eq!(clock.at(0.0, |a| a + 1.0), (0.0, 1.0, true));
    assert_eq!(clock.at(0.5, |a| a + 1.0), (0.0, 1.0, false));
    assert_eq!(clock.at(1.25, |a| a + 1.0), (1.25, 2.25, true));
    // Set back, it steps there at once.
    assert_eq!(clock.at(0.75, |a| a + 1.0), (0.75, 1.75, true));
}

/// Decor's tiers are angles: the model's and the impostor's edges are their
/// shares of the view's cell (`CELL`), whatever the clip level's height.
#[test]
fn decor_edges_cell_shares() {
    use crate::domain::view::frame::pose::view_resize;
    use crate::domain::view::param::{DECOR_IMPOSTOR_CELLS, DECOR_MODEL_CELLS};
    let _g = serial();
    let edges = (DECOR_MODEL_CELLS * CELL, DECOR_IMPOSTOR_CELLS * CELL);
    for h in [600.0, 844.0, 1080.0, 1440.0, 2160.0] {
        view_resize(h * 16.0 / 9.0, h);
        let (model, impostor) = (decor_model_angle(), decor_impostor_angle());
        assert!((model - edges.0).abs() < 1e-15, "{h}: {model}");
        assert!((impostor - edges.1).abs() < 1e-15, "{h}: {impostor}");
    }
}

/// A kind is generated at the leaves' clip level and at each coarser one where
/// a variant stands as itself (`scatter::stands_at`): its clip levels run from
/// the leaves' up, unbroken, never at one whose cell its largest box keeps no
/// eighth of, as a folded cube of it would keep none.
#[test]
fn decor_clip_levels_by_size() {
    use pxl_spec::space::coord::zoom::{units, UNIT_ZOOM};
    let leaf = 1 << (LEAF_CELL - FIRST_CELL);
    // From a tuft to a ship, as it shows.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in zooms_by(|_| extent).iter().enumerate() {
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
                        .map(|&s| units(UNIT_ZOOM) * f64::from(s))
                        .product::<f64>()
                })
                .fold(0.0, f64::max);
            assert!(
                coarsest == LEAF_CELL
                    || 8.0 * volume >= units(coarsest) * units(coarsest) * units(coarsest),
                "{:?}",
                d.id
            );
        }
    }
}
