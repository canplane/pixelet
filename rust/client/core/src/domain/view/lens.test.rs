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
