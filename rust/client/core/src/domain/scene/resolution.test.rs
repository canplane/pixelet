// pxl-core/domain/scene/resolution.test.rs

use super::*;
use crate::domain::setting::settings_mut;

/// A window's sample, its per-pixel part and its rest each given as a share
/// of the window's budget.
fn cost(px: f64, rest: f64) -> Cost {
    let w = param::RES_WINDOW_MS;
    Cost {
        px_ms: px * w,
        rest_ms: rest * w,
    }
}

/// A moving frame on a display of `ms`, `ms` after the last, so its budget at
/// the rate's first interval is `ms`.
fn pace(ms: f64) -> Pace {
    Pace {
        gap_ms: ms,
        display_ms: ms,
        motion: Motion::Moving,
    }
}

/// Adds one frame of `pace` costing `cost`, and answers whether it moved the
/// scale.
fn stepped(k: &mut Judge, cost: Option<Cost>, pace: &Pace, timed: bool) -> bool {
    let step = k.step;
    k.judge(cost, pace, timed);
    k.step != step
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Runs `n` windows of one frame each, costing `load` budgets, all of it per
/// pixel, and returns how many moved the scale.
fn windows(k: &mut Judge, load: f64, n: u32, timed: bool) -> u32 {
    windows_of(k, cost(load, 0.0), n, timed)
}

/// Runs `n` windows of one frame each, costing `s`, and returns how many moved
/// the scale.
fn windows_of(k: &mut Judge, s: Cost, n: u32, timed: bool) -> u32 {
    (0..n)
        .filter(|_| stepped(k, Some(s), &pace(param::RES_WINDOW_MS), timed))
        .count() as u32
}

/// With timestamps, a window over its budget steps down at once. The settling
/// windows after a change are not judged. The step above is taken again only
/// after `RES_UP_WINDOW_CNT` windows in which it would fit.
#[test]
fn timed_steps() {
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 1);
    assert_eq!(
        windows(&mut k, 1.5, param::RES_SETTLE_WINDOW_CNT, true),
        0,
        "settling"
    );
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(k.step, 2);
    // Fits at this step, but the step above would not.
    assert_eq!(windows(&mut k, 0.8, 20, true), 0);
    assert_eq!(windows(&mut k, 0.3, param::RES_UP_WINDOW_CNT - 1, true), 0);
    assert_eq!(windows(&mut k, 0.3, 1, true), 1);
    assert_eq!(k.step, 1);
}

/// With timestamps, a window's length counts every frame's budget, whether or
/// not a read landed with the frame. Reads on one frame in three end and judge
/// the window as soon as reads on every frame would, by the frames that took
/// them. A window in which no read landed is not judged.
#[test]
fn timed_windows() {
    // A 60 Hz display's interval.
    let frame = 1000.0 / 60.0;
    let frames = (param::RES_WINDOW_MS / frame).ceil() as u32;
    let mut k = Judge::NEW;
    let changed = (0..frames)
        .filter(|i| {
            let s = Cost {
                px_ms: 1.5 * frame,
                ..Cost::default()
            };
            stepped(&mut k, (i % 3 == 0).then_some(s), &pace(frame), true)
        })
        .count();
    assert_eq!((changed, k.step), (1, 1), "one window's frames step down");
    let mut k = Judge::NEW;
    let changed = (0..3 * frames)
        .filter(|_| stepped(&mut k, None, &pace(frame), true))
        .count();
    assert_eq!(
        (changed, k.step, k.since),
        (0, 0, u32::MAX),
        "no read, no judgment"
    );
}

/// Without timestamps, a step up is a probe taken after a run of quiet
/// windows. A probe that goes over within its trial steps back and doubles the
/// wait. A probe that outlasts its trial stands.
#[test]
fn untimed_probe_backoff() {
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, param::SCALE_MISSED + 0.5, 1, false), 1);
    assert_eq!(k.step, 1);
    let settle = param::RES_SETTLE_WINDOW_CNT;
    assert_eq!(
        windows(
            &mut k,
            1.0,
            settle + param::SCALE_PROBE_WINDOW_CNT - 1,
            false
        ),
        0
    );
    assert_eq!(windows(&mut k, 1.0, 1, false), 1);
    assert_eq!((k.step, k.on_trial), (0, true));
    assert_eq!(windows(&mut k, 1.0, settle, false), 0);
    assert_eq!(
        windows(&mut k, param::SCALE_MISSED + 0.5, 1, false),
        1,
        "the probe fails"
    );
    assert_eq!(
        (k.step, k.on_trial, k.hold(false)),
        (1, false, 2 * param::SCALE_PROBE_WINDOW_CNT)
    );
    assert_eq!(
        windows(
            &mut k,
            1.0,
            settle + 2 * param::SCALE_PROBE_WINDOW_CNT,
            false
        ),
        1
    );
    assert_eq!(
        windows(&mut k, 1.0, param::RES_TRUST_WINDOW_CNT + 1, false),
        0
    );
    assert_eq!(
        (k.step, k.on_trial, k.hold(false)),
        (0, false, param::SCALE_PROBE_WINDOW_CNT),
        "the probe outlasted its trust"
    );
}

/// With timestamps, two steps do not swing when the step above is predicted
/// to fit and does not. Each fall back within the trial doubles the windows
/// before the next step up, up to `RES_UP_WINDOW_CAP`. A step up that
/// outlasts its trial stands, and the wait returns to its first length.
#[test]
fn timed_up_backoff() {
    let settle = param::RES_SETTLE_WINDOW_CNT;
    let mut k = Judge::NEW;
    k.step = 1;
    // Each round steps up after its wait, goes over within the trial and
    // falls back, so the wait doubles and then doubles again.
    let mut wait = param::RES_UP_WINDOW_CNT;
    for _ in 0..2 {
        assert_eq!(windows(&mut k, 0.3, wait - 1, true), 0);
        assert_eq!(windows(&mut k, 0.3, 1, true), 1);
        assert_eq!((k.step, k.on_trial), (0, true));
        assert_eq!(windows(&mut k, 0.3, settle, true), 0);
        assert_eq!(windows(&mut k, 1.5, 1, true), 1, "the step up falls back");
        assert_eq!((k.step, k.on_trial), (1, false));
        assert_eq!(windows(&mut k, 0.3, settle, true), 0);
        wait *= 2;
        assert_eq!(k.hold(true), wait);
    }
    // The wait has a cap.
    k.backoff = 1 << 20;
    assert_eq!(k.hold(true), param::RES_UP_WINDOW_CAP);
    // A step up that outlasts its trial stands, and the wait returns to its
    // first length.
    k.backoff = 2;
    assert_eq!(windows(&mut k, 0.3, 2 * param::RES_UP_WINDOW_CNT, true), 1);
    assert_eq!(k.step, 0);
    assert_eq!(
        windows(&mut k, 0.3, param::RES_TRUST_WINDOW_CNT + 1, true),
        0
    );
    assert_eq!(
        (k.step, k.on_trial, k.hold(true)),
        (0, false, param::RES_UP_WINDOW_CNT)
    );
}

/// A pause in motion starts a fresh window but keeps the quiet windows judged
/// before it, so a drag, a pause and another drag step up as one drag would,
/// with timestamps or without.
#[test]
fn pause_keeps_windows() {
    let settle = param::RES_SETTLE_WINDOW_CNT;
    let mut k = Judge::NEW;
    assert_eq!(windows(&mut k, 1.5, 1, true), 1);
    assert_eq!(
        windows(&mut k, 0.3, settle + param::RES_UP_WINDOW_CNT - 1, true),
        0
    );
    k.judge(
        Some(cost(0.3, 0.0)),
        &pace(0.5 * param::RES_WINDOW_MS),
        true,
    );
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
        windows(
            &mut k,
            1.0,
            settle + param::SCALE_PROBE_WINDOW_CNT - 1,
            false
        ),
        0
    );
    k.restart();
    assert_eq!(
        windows(&mut k, 1.0, 1, false),
        1,
        "the probe's hold outlasts the pause"
    );
    assert_eq!((k.step, k.on_trial), (0, true));
}

/// With timestamps, windows over their budget step the scale down to
/// `RES_MIN`, each change followed by its settling windows, and at `RES_MIN`
/// the scale holds.
#[test]
fn down_scale_to_its_floor() {
    let (over, settle) = (cost(0.6, 0.9), param::RES_SETTLE_WINDOW_CNT);
    let mut k = Judge::NEW;
    for step in 1..=param::RES_STEP_CNT {
        assert_eq!(windows_of(&mut k, over, 1, true), 1);
        assert_eq!(k.step, step);
        assert_eq!(windows_of(&mut k, over, settle, true), 0, "settling");
    }
    assert_eq!(windows_of(&mut k, over, settle + 10, true), 0, "past it");
}

/// With timestamps, the scale steps up after `RES_UP_WINDOW_CNT` windows in
/// which the step above fits.
#[test]
fn up_scale() {
    let mut k = Judge {
        step: 2,
        ..Judge::NEW
    };
    let room = cost(0.1, 0.25);
    assert_eq!(
        windows_of(&mut k, room, param::RES_UP_WINDOW_CNT - 1, true),
        0
    );
    assert_eq!(windows_of(&mut k, room, 1, true), 1);
    assert_eq!(k.step, 1);
}

/// With timestamps, the step above is judged by the whole frame it would
/// cost: the rest as it is, plus the per-pixel time grown by the step's
/// pixels. A step whose per-pixel time alone would fit still waits.
#[test]
fn step_up_judges_total() {
    let mut k = Judge {
        step: 1,
        ..Judge::NEW
    };
    // A step up grows the per-pixel time from 0.2 to 0.28. That alone is
    // under the 0.9 the band leaves of the budget, but with the rest's 0.7 it
    // is over.
    assert_eq!(windows_of(&mut k, cost(0.2, 0.7), 20, true), 0);
    assert_eq!(k.step, 1);
}

/// The load predicted at another step keeps the rest as it is and scales the
/// per-pixel time by the pixel count there over here. A longer budget spreads
/// the load over it.
#[test]
fn predicted_by_part() {
    let k = Judge {
        step: param::RES_STEP_CNT,
        ..Judge::NEW
    };
    let (s, w) = (cost(0.2, 0.4), param::RES_WINDOW_MS);
    assert!(near(k.predicted(s, w, k.step), 0.6));
    let px = 1.0 / (param::RES_MIN * param::RES_MIN);
    assert!(near(k.predicted(s, w, 0), 0.4 + 0.2 * px));
    assert!(near(k.predicted(s, 2.0 * w, k.step), 0.3));
}

/// Without timestamps, only the scale moves, and at `RES_MIN` a window over
/// its budget moves nothing.
#[test]
fn untimed_scale_alone() {
    let (over, settle) = (param::SCALE_MISSED + 0.5, param::RES_SETTLE_WINDOW_CNT);
    let mut k = Judge::NEW;
    for _ in 0..param::RES_STEP_CNT {
        assert_eq!(windows(&mut k, over, 1, false), 1);
        assert_eq!(windows(&mut k, over, settle, false), 0);
    }
    assert_eq!(windows(&mut k, over, settle + 10, false), 0);
    assert_eq!(k.step, param::RES_STEP_CNT);
}

/// A frame's budget is its target interval, never less than one interval at
/// `RATE_MAX_HZ`: two intervals at 120 Hz, and one at `RATE_MAX_HZ` at 90 Hz,
/// at 60 Hz and before the display is measured.
#[test]
fn the_budget_is_the_target_to_the_cap() {
    settings_mut().res = ResMode::Auto;
    let cap_ms = 1000.0 / param::RATE_MAX_HZ;
    assert_eq!(budget(1000.0 / 120.0), 2.0 * (1000.0 / 120.0), "120 Hz");
    for hz in [90.0, 60.0] {
        assert_eq!(budget(1000.0 / hz), cap_ms, "{hz} Hz");
    }
    assert_eq!(budget(0.0), cap_ms, "not measured");
}
