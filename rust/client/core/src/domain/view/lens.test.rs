// pxl-core/domain/view/lens.test.rs

use super::*;

/// The reference `StepClock` is held to: a step clock written out plainly.
/// It takes a new reading at `t` when `t` reached the next step after
/// the held reading `at`, or went back before it, and shows the held reading.
fn stepped_as_written(at: &mut Option<f64>, t: f64, next: impl Fn(f64) -> f64) -> (f64, f64) {
    if at.is_none_or(|a| t >= next(a) || t < a) {
        *at = Some(t);
    }
    let a = at.unwrap_or(t);
    (a, next(a))
}

#[test]
fn step_clock_steps() {
    // Wall clock readings that move on and once step back, under three rules
    // for the next step: a fixed interval, one that grows with the reading,
    // and none at all.
    let readings = [
        1_000.0, 1_016.7, 1_033.3, 1_050.0, 1_066.6, 1_100.0, 1_040.0, 1_041.0, 1_200.0, 1_200.0,
    ];
    let nexts: [fn(f64) -> f64; 3] = [|a| a + 50.0, |a| a + a / 40.0, |a| a + f64::INFINITY];
    for next in nexts {
        let (mut clock, mut before) = (StepClock::default(), None);
        for t in readings {
            let (shown, due, _) = clock.at(t, next);
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
fn step_clock_reports() {
    let mut clock = StepClock::default();
    assert_eq!(clock.at(0.0, |a| a + 1.0), (0.0, 1.0, true));
    assert_eq!(clock.at(0.5, |a| a + 1.0), (0.0, 1.0, false));
    assert_eq!(clock.at(1.25, |a| a + 1.0), (1.25, 2.25, true));
    // A clock set back steps to the new reading at once.
    assert_eq!(clock.at(0.75, |a| a + 1.0), (0.75, 1.75, true));
}

/// The edges between decor's lods are angles. The model's edge and the
/// impostor's edge stay fixed shares of the view's cell (`CELL`) whatever the
/// screen's height.
#[test]
fn decor_edges_cell_shares() {
    use crate::domain::view::frame::pose::view_resize;
    use crate::domain::view::param::DECOR_IMPOSTOR_CELLS;
    let edges = (f64::from(SPANS) * CELL, DECOR_IMPOSTOR_CELLS * CELL);
    for h in [600.0, 844.0, 1080.0, 1440.0, 2160.0] {
        view_resize(h * 16.0 / 9.0, h);
        let (model, impostor) = (decor_model_angle(), decor_impostor_angle());
        assert!((model - edges.0).abs() < 1e-15, "{h}: {model}");
        assert!((impostor - edges.1).abs() < 1e-15, "{h}: {impostor}");
    }
}

/// A decor is generated at the leaves' clip level and at each coarser one where
/// a variant shows as itself (`scatter::shows_at`). Its clip levels run
/// unbroken from the leaves' upward. None of them has cells of which the
/// decor's largest box fills less than an eighth, since a folded cube of that
/// box would keep nothing there.
#[test]
fn decor_clip_levels_by_size() {
    use pxl_spec::space::coord::zoom::{UNIT_ZOOM, units};
    let leaf = 1 << (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM);
    // Extents from a tuft of grass to a ship.
    for extent in [0.5, 2.0, 8.0, 40.0, 400.0] {
        for (k, &bits) in zooms_by(|_| extent).iter().enumerate() {
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
            let volume = (0..d.row.variant_cnt as usize)
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
