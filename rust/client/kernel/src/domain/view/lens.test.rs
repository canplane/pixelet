// pxl-kernel/domain/view/lens.test.rs

use super::*;

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
fn a_stepped_clock_steps_where_the_clocks_stepped_before() {
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
fn a_stepped_clock_says_when_it_stepped() {
    let mut clock = Stepped::default();
    assert_eq!(clock.at(0.0, |a| a + 1.0), (0.0, 1.0, true));
    assert_eq!(clock.at(0.5, |a| a + 1.0), (0.0, 1.0, false));
    assert_eq!(clock.at(1.25, |a| a + 1.0), (1.25, 2.25, true));
    // Set back, it steps there at once.
    assert_eq!(clock.at(0.75, |a| a + 1.0), (0.75, 1.75, true));
}
