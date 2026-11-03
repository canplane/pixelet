// pxl-kernel/domain/view/frame/resolution.test.rs

use super::*;

/// A window's sample, each part a share of its budget: per pixel, per
/// lattice and the rest.
fn cost(px: f64, cell: f64, rest: f64) -> Cost {
    let w = param::RES_WINDOW_MS;
    Cost {
        px_ms: px * w,
        cell_ms: cell * w,
        rest_ms: rest * w,
    }
}

/// Windows of `load` budgets each, all of it per pixel, judged as one frame
/// a window.
fn windows(k: &mut Judge, load: f64, n: u32, timed: bool) -> u32 {
    windows_of(k, cost(load, 0.0, 0.0), n, timed)
}

/// Windows of `s` each, judged as one frame a window: how many changed a
/// knob.
fn windows_of(k: &mut Judge, s: Cost, n: u32, timed: bool) -> u32 {
    (0..n)
        .filter(|_| k.judge(Some(s), param::RES_WINDOW_MS, timed))
        .count() as u32
}

/// Timed, a window over its budget steps down at once; a change is not
/// judged again before its settling windows, and the step above is taken
/// back only after `RES_UP_WINDOWS` windows in which it fits.
#[test]
fn timed_steps() {
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 1);
    assert_eq!(
        windows(&mut k, 1.5, param::RES_SETTLE_WINDOWS, true),
        0,
        "settling"
    );
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 2);
    // Fits at this step, but the step above would not.
    assert_eq!(windows(&mut k, 0.8, 20, true), 0);
    assert_eq!(windows(&mut k, 0.3, param::RES_UP_WINDOWS - 1, true), 0);
    assert_eq!(windows(&mut k, 0.3, 1, true), 1);
    assert_eq!(k.step, 1);
}

/// Timed, a window is its frames' budgets whether or not a read landed with
/// them: reads on one frame in three judge it as soon as every frame would,
/// by the frames that took them, and a window none landed in is not judged.
#[test]
fn timed_windows() {
    let frame = param::RES_BUDGET_FLOOR_MS;
    let frames = (param::RES_WINDOW_MS / frame).ceil() as u32;
    let mut k = Judge::NEW;
    let changed = (0..frames)
        .filter(|i| {
            let s = Cost {
                px_ms: 1.5 * frame,
                ..Cost::default()
            };
            k.judge((i % 3 == 0).then_some(s), frame, true)
        })
        .count();
    assert_eq!((changed, k.step), (1, 1), "one window's frames step down");
    let mut k = Judge::NEW;
    let changed = (0..3 * frames)
        .filter(|_| k.judge(None, frame, true))
        .count();
    assert_eq!(
        (changed, k.step, k.since),
        (0, 0, u32::MAX),
        "no read, no judgement"
    );
}

/// Untimed, a step up is a probe after a hold of quiet windows; one that
/// goes over within its trust steps back and doubles the hold, and one that
/// outlasts it stands.
#[test]
fn untimed_probe_backoff() {
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1);
    assert_eq!(k.step, 1);
    let settle = param::RES_SETTLE_WINDOWS;
    assert_eq!(
        windows(&mut k, 1.0, settle + param::SCALE_PROBE_WINDOWS - 1, false),
        0
    );
    assert_eq!(windows(&mut k, 1.0, 1, false), 1);
    assert_eq!((k.step, k.probing), (0, true));
    assert_eq!(windows(&mut k, 1.0, settle, false), 0);
    assert_eq!(
        windows(&mut k, param::SCALE_MISSED + 0.5, 1, false),
        1,
        "the probe fails"
    );
    assert_eq!(
        (k.step, k.probing, k.hold),
        (1, false, 2 * param::SCALE_PROBE_WINDOWS)
    );
    assert_eq!(
        windows(&mut k, 1.0, settle + 2 * param::SCALE_PROBE_WINDOWS, false),
        1
    );
    assert_eq!(
        windows(&mut k, 1.0, param::SCALE_TRUST_WINDOWS + 1, false),
        0
    );
    assert_eq!(
        (k.step, k.probing),
        (0, false),
        "the probe outlasted its trust"
    );
}

/// A pause in moving starts a fresh window but keeps the quiet windows
/// judged before it: a drag, a pause and a drag step up as one drag would,
/// timed or untimed.
#[test]
fn pause_keeps_windows() {
    let settle = param::RES_SETTLE_WINDOWS;
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(
        windows(&mut k, 0.3, settle + param::RES_UP_WINDOWS - 1, true),
        0
    );
    k.judge(Some(cost(0.3, 0.0, 0.0)), 0.5 * param::RES_WINDOW_MS, true);
    k.restart();
    assert_eq!(
        windows(&mut k, 0.3, 1, true),
        1,
        "the drag after the pause finishes the run"
    );
    assert_eq!(k.step, 0);

    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1);
    assert_eq!(
        windows(&mut k, 1.0, settle + param::SCALE_PROBE_WINDOWS - 1, false),
        0
    );
    k.restart();
    assert_eq!(
        windows(&mut k, 1.0, 1, false),
        1,
        "the probe's hold outlasts the pause"
    );
    assert_eq!((k.step, k.probing), (0, true));
}

/// Timed, a window over its budget steps the scale down to `SCALE_MIN`
/// first, then the lattice to `LATTICE_MAX`, a knob a step, each change
/// followed by its settling windows, and past both holds.
#[test]
fn down_scale_then_lattice() {
    let (over, settle) = (cost(0.6, 0.3, 0.6), param::RES_SETTLE_WINDOWS);
    let mut k = Judge::NEW;
    for step in 1..=param::RES_STEPS {
        assert_eq!(windows_of(&mut k, over, 1, true), 1);
        assert_eq!((k.step, k.lattice), (step, 1));
        assert_eq!(windows_of(&mut k, over, settle, true), 0, "settling");
    }
    assert_eq!(windows_of(&mut k, over, 1, true), 1);
    assert_eq!((k.step, k.lattice), (param::RES_STEPS, LATTICE_MAX));
    assert_eq!(windows_of(&mut k, over, settle + 10, true), 0, "past both");
}

/// Timed, room steps the lattice up first, then the scale, each after
/// `RES_UP_WINDOWS` windows in which the step above fits.
#[test]
fn up_lattice_then_scale() {
    let mut k = Judge {
        step: 2,
        lattice: LATTICE_MAX,
        ..Judge::NEW
    };
    let room = cost(0.1, 0.05, 0.2);
    assert_eq!(windows_of(&mut k, room, param::RES_UP_WINDOWS - 1, true), 0);
    assert_eq!(windows_of(&mut k, room, 1, true), 1);
    assert_eq!((k.step, k.lattice), (2, 1), "the lattice first");
    let settle = param::RES_SETTLE_WINDOWS;
    assert_eq!(
        windows_of(&mut k, room, settle + param::RES_UP_WINDOWS, true),
        1
    );
    assert_eq!((k.step, k.lattice), (1, 1), "then the scale");
}

/// Timed, the step above is judged by the whole frame it would cost: the
/// rest as it is, the per-pixel time by its pixels and the lattice's by its
/// cells, so a step whose per-pixel time alone fits waits.
#[test]
fn step_up_judges_total() {
    let mut k = Judge {
        step: 1,
        ..Judge::NEW
    };
    // The per-pixel time 0.2 grows to 0.28 a step up, under the budget's
    // 0.9 alone, but over it with the rest's 0.7.
    assert_eq!(windows_of(&mut k, cost(0.2, 0.0, 0.7), 20, true), 0);
    assert_eq!(k.step, 1);
    let mut k = Judge {
        step: param::RES_STEPS,
        lattice: LATTICE_MAX,
        ..Judge::NEW
    };
    // The lattice's 0.2 grows fourfold at the cell's: 0.3 + 0.8 is over.
    assert_eq!(windows_of(&mut k, cost(0.1, 0.2, 0.2), 20, true), 0);
    assert_eq!(
        windows_of(&mut k, cost(0.1, 0.1, 0.2), param::RES_UP_WINDOWS, true),
        1
    );
    assert_eq!(k.lattice, 1);
}

/// What a step would cost: the rest as it is, the per-pixel time by the
/// pixels there over these, the lattice's by its cells there over these.
#[test]
fn predicted_by_part() {
    let k = Judge {
        step: param::RES_STEPS,
        lattice: LATTICE_MAX,
        ..Judge::NEW
    };
    let (s, w) = (cost(0.2, 0.1, 0.3), param::RES_WINDOW_MS);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(near(k.predicted(s, w, (k.step, k.lattice)), 0.6));
    assert!(near(k.predicted(s, w, (k.step, 1)), 0.3 + 0.2 + 0.4));
    let px = 1.0 / (param::SCALE_MIN * param::SCALE_MIN);
    assert!(near(
        k.predicted(s, w, (0, k.lattice)),
        0.3 + 0.2 * px + 0.1
    ));
}

/// Untimed, the scale alone moves: past `SCALE_MIN` a window over holds,
/// and the lattice stays the cell's own.
#[test]
fn untimed_scale_alone() {
    let (over, settle) = (param::SCALE_MISSED + 0.5, param::RES_SETTLE_WINDOWS);
    let mut k = Judge::NEW;
    for _ in 0..param::RES_STEPS {
        assert_eq!(windows(&mut k, over, 1, false), 1);
        assert_eq!(windows(&mut k, over, settle, false), 0);
    }
    assert_eq!(windows(&mut k, over, settle + 10, false), 0);
    assert_eq!((k.step, k.lattice), (param::RES_STEPS, 1));
}
