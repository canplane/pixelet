// pxl-kernel/domain/view/frame/resolution.test.rs

use super::*;

/// Windows of `load` budgets each, judged as one frame a window.
fn windows(k: &mut Scale, load: f64, n: u32, timed: bool) -> u32 {
    (0..n).filter(|_| k.judge(load * param::SCALE_WINDOW_MS, param::SCALE_WINDOW_MS, timed)).count() as u32
}

/// Timed, a window over its budget steps down at once; a change is not
/// judged again before its settling windows, and the step above is taken
/// back only after `SCALE_UP_WINDOWS` windows in which it fits.
#[test]
fn timed_steps_down_at_once_and_up_once_the_step_above_fits() {
    let mut k = Scale::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 1);
    assert_eq!(windows(&mut k, 1.5, param::SCALE_SETTLE_WINDOWS, true), 0, "settling");
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 2);
    // Fits at this step, but the step above would not.
    assert_eq!(windows(&mut k, 0.8, 20, true), 0);
    assert_eq!(windows(&mut k, 0.3, param::SCALE_UP_WINDOWS - 1, true), 0);
    assert_eq!(windows(&mut k, 0.3, 1, true), 1);
    assert_eq!(k.step, 1);
}

/// Untimed, a step up is a probe after a hold of quiet windows; one that
/// goes over within its trust steps back and doubles the hold, and one that
/// outlasts it stands.
#[test]
fn untimed_probes_up_and_a_failed_probe_doubles_the_hold() {
    let mut k = Scale::NEW;
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1);
    assert_eq!(k.step, 1);
    let settle = param::SCALE_SETTLE_WINDOWS;
    assert_eq!(windows(&mut k, 1.0, settle + param::SCALE_PROBE_WINDOWS - 1, false), 0);
    assert_eq!(windows(&mut k, 1.0, 1, false), 1);
    assert_eq!((k.step, k.probing), (0, true));
    assert_eq!(windows(&mut k, 1.0, settle, false), 0);
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1, "the probe fails");
    assert_eq!((k.step, k.probing, k.hold), (1, false, 2 * param::SCALE_PROBE_WINDOWS));
    assert_eq!(windows(&mut k, 1.0, settle + 2 * param::SCALE_PROBE_WINDOWS, false), 1);
    assert_eq!(windows(&mut k, 1.0, param::SCALE_TRUST_WINDOWS + 1, false), 0);
    assert_eq!((k.step, k.probing), (0, false), "the probe outlasted its trust");
}

/// A pause in moving starts a fresh window but keeps the quiet windows
/// judged before it: a drag, a pause and a drag step up as one drag would,
/// timed or untimed.
#[test]
fn a_pause_keeps_the_quiet_windows_judged() {
    let settle = param::SCALE_SETTLE_WINDOWS;
    let mut k = Scale::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(windows(&mut k, 0.3, settle + param::SCALE_UP_WINDOWS - 1, true), 0);
    k.judge(0.3 * param::SCALE_WINDOW_MS, 0.5 * param::SCALE_WINDOW_MS, true);
    k.restart();
    assert_eq!(windows(&mut k, 0.3, 1, true), 1, "the drag after the pause finishes the run");
    assert_eq!(k.step, 0);

    let mut k = Scale::NEW;
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1);
    assert_eq!(windows(&mut k, 1.0, settle + param::SCALE_PROBE_WINDOWS - 1, false), 0);
    k.restart();
    assert_eq!(windows(&mut k, 1.0, 1, false), 1, "the probe's hold outlasts the pause");
    assert_eq!((k.step, k.probing), (0, true));
}
