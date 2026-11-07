// pxl-core/domain/scene/resolution.test.rs

use super::*;

/// A window's sample, each part a share of its budget: per pixel and the
/// rest.
fn cost(px: f64, rest: f64) -> Cost {
    let w = param::RES_WINDOW_MS;
    Cost {
        px_ms: px * w,
        rest_ms: rest * w,
    }
}

/// A frame paced `ms` apart on a display too slow for the rate to step: the
/// scale is what moves.
fn paced(ms: f64) -> Pace {
    Pace {
        refresh_ms: 1000.0,
        paced_ms: ms,
        gap_ms: ms,
        moving: true,
    }
}

/// A frame `refreshes` of a `hz` display apart.
fn on(hz: f64, refreshes: f64) -> Pace {
    Pace {
        refresh_ms: 1000.0 / hz,
        paced_ms: refreshes * 1000.0 / hz,
        gap_ms: 0.0,
        moving: true,
    }
}

/// A window of frames paced as `p`, each `load` of its budget, per pixel:
/// the knobs it moved to, where it moved one, the interval moved to in `p`,
/// as the next grant says it.
fn window(k: &mut Judge, p: &mut Pace, load: f64, timed: bool) -> Option<Knobs> {
    let s = Cost {
        px_ms: load * budget(p.paced_ms),
        ..Cost::default()
    };
    loop {
        let to = k.judge(Some(s), p, timed);
        if k.window_ms == 0.0 {
            if let Some((paced_ms, ..)) = to {
                p.paced_ms = paced_ms;
            }
            return to;
        }
    }
}

/// `window` over and over, at most `n` times: the knobs the first that moved
/// one moved to.
fn until_moved(k: &mut Judge, p: &mut Pace, load: f64, n: u32) -> Option<Knobs> {
    (0..n).find_map(|_| window(k, p, load, true))
}

/// The windows after a change that are not judged.
fn settle(k: &mut Judge, p: &mut Pace, load: f64, timed: bool) {
    for _ in 0..param::RES_SETTLE_WINDOWS {
        assert_eq!(window(k, p, load, timed), None, "settling");
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Windows of `load` budgets each, all of it per pixel, judged as one frame
/// a window.
fn windows(k: &mut Judge, load: f64, n: u32, timed: bool) -> u32 {
    windows_of(k, cost(load, 0.0), n, timed)
}

/// Windows of `s` each, judged as one frame a window: how many changed a
/// knob.
fn windows_of(k: &mut Judge, s: Cost, n: u32, timed: bool) -> u32 {
    (0..n)
        .filter(|_| {
            k.judge(Some(s), &paced(param::RES_WINDOW_MS), timed)
                .is_some()
        })
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
            k.judge((i % 3 == 0).then_some(s), &paced(frame), true)
                .is_some()
        })
        .count();
    assert_eq!((changed, k.step), (1, 1), "one window's frames step down");
    let mut k = Judge::NEW;
    let changed = (0..3 * frames)
        .filter(|_| k.judge(None, &paced(frame), true).is_some())
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
    assert_eq!((k.step, k.on_trial), (0, true));
    assert_eq!(windows(&mut k, 1.0, settle, false), 0);
    assert_eq!(
        windows(&mut k, param::SCALE_MISSED + 0.5, 1, false),
        1,
        "the probe fails"
    );
    assert_eq!(
        (k.step, k.on_trial, k.hold(false)),
        (1, false, 2 * param::SCALE_PROBE_WINDOWS)
    );
    assert_eq!(
        windows(&mut k, 1.0, settle + 2 * param::SCALE_PROBE_WINDOWS, false),
        1
    );
    assert_eq!(windows(&mut k, 1.0, param::RES_TRUST_WINDOWS + 1, false), 0);
    assert_eq!(
        (k.step, k.on_trial, k.hold(false)),
        (0, false, param::SCALE_PROBE_WINDOWS),
        "the probe outlasted its trust"
    );
}

/// Timed, two steps do not swing where the step above is predicted to fit
/// and does not: each fall within the trust doubles the windows before the
/// next step up, up to the most, and one that outlasts the trust stands and
/// the wait is as it was.
#[test]
fn timed_up_backoff() {
    let settle = param::RES_SETTLE_WINDOWS;
    let mut k = Judge::NEW;
    k.step = 1;
    // Up after its wait, over within the trust, back: twice the wait, then
    // four times.
    let mut wait = param::RES_UP_WINDOWS;
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
    // The most it waits.
    k.backoff = 1 << 20;
    assert_eq!(k.hold(true), param::RES_UP_WINDOWS_MAX);
    // A step up that outlasts its trust stands, and the wait is as it was.
    k.backoff = 2;
    assert_eq!(windows(&mut k, 0.3, 2 * param::RES_UP_WINDOWS, true), 1);
    assert_eq!(k.step, 0);
    assert_eq!(windows(&mut k, 0.3, param::RES_TRUST_WINDOWS + 1, true), 0);
    assert_eq!(
        (k.step, k.on_trial, k.hold(true)),
        (0, false, param::RES_UP_WINDOWS)
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
    k.judge(
        Some(cost(0.3, 0.0)),
        &paced(0.5 * param::RES_WINDOW_MS),
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
        windows(&mut k, 1.0, settle + param::SCALE_PROBE_WINDOWS - 1, false),
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

/// Timed, a window over its budget steps the scale down to `RES_MIN`, each
/// change followed by its settling windows, and past it holds.
#[test]
fn down_scale_to_its_floor() {
    let (over, settle) = (cost(0.6, 0.9), param::RES_SETTLE_WINDOWS);
    let mut k = Judge::NEW;
    for step in 1..=param::RES_STEPS {
        assert_eq!(windows_of(&mut k, over, 1, true), 1);
        assert_eq!(k.step, step);
        assert_eq!(windows_of(&mut k, over, settle, true), 0, "settling");
    }
    assert_eq!(windows_of(&mut k, over, settle + 10, true), 0, "past it");
}

/// Timed, room steps the scale up, after `RES_UP_WINDOWS` windows in which
/// the step above fits.
#[test]
fn up_scale() {
    let mut k = Judge {
        step: 2,
        ..Judge::NEW
    };
    let room = cost(0.1, 0.25);
    assert_eq!(windows_of(&mut k, room, param::RES_UP_WINDOWS - 1, true), 0);
    assert_eq!(windows_of(&mut k, room, 1, true), 1);
    assert_eq!(k.step, 1);
}

/// Timed, the step above is judged by the whole frame it would cost: the
/// rest as it is and the per-pixel time by its pixels, so a step whose
/// per-pixel time alone fits waits.
#[test]
fn step_up_judges_total() {
    let mut k = Judge {
        step: 1,
        ..Judge::NEW
    };
    // The per-pixel time 0.2 grows to 0.28 a step up, under the budget's
    // 0.9 alone, but over it with the rest's 0.7.
    assert_eq!(windows_of(&mut k, cost(0.2, 0.7), 20, true), 0);
    assert_eq!(k.step, 1);
}

/// What a step would cost: the rest as it is, the per-pixel time by the
/// pixels there over these.
#[test]
fn predicted_by_part() {
    let k = Judge {
        step: param::RES_STEPS,
        ..Judge::NEW
    };
    let (s, w) = (cost(0.2, 0.4), param::RES_WINDOW_MS);
    let p = paced(w);
    assert!(near(k.predicted(s, w, &p, (w, k.step)), 0.6));
    let px = 1.0 / (param::RES_MIN * param::RES_MIN);
    assert!(near(k.predicted(s, w, &p, (w, 0)), 0.4 + 0.2 * px));
    assert!(near(k.predicted(s, w, &p, (2.0 * w, k.step)), 0.3));
}

/// The rate steps within what the display and `RATE_MIN_HZ` leave: a refresh
/// at a time, never past the floor, and turns are slowed only past the
/// display's own divisor (@xpute/runtime sched/frame_pacing.ts `pace_ms`).
#[test]
fn rate_by_display() {
    for (hz, own, floor) in [
        (120.0, 2.0, 4.0),
        (60.0, 1.0, 2.0),
        (144.0, 2.0, 4.0),
        (165.0, 2.0, 5.0),
        (90.0, 1.0, 3.0),
        (30.0, 1.0, 1.0),
    ] {
        assert!(!on(hz, own).slowed(), "{hz} Hz at its own rate");
        assert!(
            on(hz, own + 1.0).slowed() || own == floor,
            "{hz} Hz past it"
        );
        assert!(!on(hz, floor).may_slow(), "{hz} Hz at its floor");
        assert!(
            own == floor || on(hz, floor - 1.0).may_slow(),
            "{hz} Hz above it"
        );
    }
}

/// Timed, a window over its budget slows the rate first, a refresh a step
/// to `RATE_MIN_HZ`, each change followed by its settling windows; only then
/// does the scale step.
#[test]
fn down_rate_then_scale() {
    let (mut k, mut p) = (Judge::NEW, on(120.0, 2.0));
    for refreshes in [3.0, 4.0] {
        assert!(window(&mut k, &mut p, 1.5, true).is_some());
        assert!(near(p.paced_ms, on(120.0, refreshes).paced_ms));
        assert_eq!(k.step, 0, "the rate first");
        settle(&mut k, &mut p, 1.5, true);
    }
    assert!(window(&mut k, &mut p, 1.5, true).is_some());
    assert!(near(p.paced_ms, on(120.0, 4.0).paced_ms), "at its floor");
    assert_eq!(k.step, 1, "then the scale");
}

/// Timed, room steps the scale up first and the rate last, to the display's
/// own, and past that there is no step.
#[test]
fn up_rate_last() {
    let mut k = Judge {
        step: 1,
        ..Judge::NEW
    };
    let mut p = on(120.0, 3.0);
    let wait = param::RES_SETTLE_WINDOWS + param::RES_UP_WINDOWS + 1;
    assert!(until_moved(&mut k, &mut p, 0.1, wait).is_some());
    assert_eq!(k.step, 0, "the scale first");
    assert!(near(p.paced_ms, on(120.0, 3.0).paced_ms));
    assert!(until_moved(&mut k, &mut p, 0.1, wait).is_some());
    assert!(near(p.paced_ms, on(120.0, 2.0).paced_ms), "the rate last");
    assert_eq!(
        until_moved(&mut k, &mut p, 0.1, 4 * wait),
        None,
        "all whole"
    );
}

/// The rate's step up is judged by the shorter budget: at 30 Hz on a 120 Hz
/// display, 0.8 of the budget is over at 40 Hz and 0.6 fits.
#[test]
fn rate_up_judges_its_budget() {
    let (mut k, mut p) = (Judge::NEW, on(120.0, 4.0));
    assert_eq!(until_moved(&mut k, &mut p, 0.8, 40), None);
    assert!(until_moved(&mut k, &mut p, 0.6, 40).is_some());
    assert!(near(p.paced_ms, on(120.0, 3.0).paced_ms));
}

/// A rate step up that goes over within its trust falls back and doubles
/// the windows before the next.
#[test]
fn rate_up_backoff() {
    let (mut k, mut p) = (Judge::NEW, on(120.0, 3.0));
    let wait = param::RES_UP_WINDOWS;
    assert!(until_moved(&mut k, &mut p, 0.3, wait).is_some());
    assert!(k.on_trial && near(p.paced_ms, on(120.0, 2.0).paced_ms));
    settle(&mut k, &mut p, 0.3, true);
    assert!(window(&mut k, &mut p, 1.5, true).is_some(), "it falls back");
    assert!(near(p.paced_ms, on(120.0, 3.0).paced_ms));
    assert_eq!((k.on_trial, k.hold(true)), (false, 2 * wait));
}

/// Untimed, room cannot be foreseen, so the rate stays and the scale moves.
#[test]
fn untimed_rate_stays() {
    let (mut k, mut p) = (Judge::NEW, on(120.0, 2.0));
    let over = param::SCALE_MISSED + 0.5;
    assert!(window(&mut k, &mut p, over, false).is_some());
    assert_eq!(k.step, 1);
    assert!(near(p.paced_ms, on(120.0, 2.0).paced_ms));
}

/// Untimed, the scale alone moves: past `RES_MIN` a window over holds,
#[test]
fn untimed_scale_alone() {
    let (over, settle) = (param::SCALE_MISSED + 0.5, param::RES_SETTLE_WINDOWS);
    let mut k = Judge::NEW;
    for _ in 0..param::RES_STEPS {
        assert_eq!(windows(&mut k, over, 1, false), 1);
        assert_eq!(windows(&mut k, over, settle, false), 0);
    }
    assert_eq!(windows(&mut k, over, settle + 10, false), 0);
    assert_eq!(k.step, param::RES_STEPS);
}
