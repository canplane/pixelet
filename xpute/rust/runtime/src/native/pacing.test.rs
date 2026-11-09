// xpute-runtime/native/pacing.test.rs

use super::*;

const PACE: Pace = Pace {
    pace_hz: 60.0,
    margin_share: 0.1,
    batch_frames: 2.0,
    settle_ms: 150.0,
    asked_ms: 0.0,
};

fn at(hz: f64, pace: Pace) -> (Pacing, f64, f64) {
    let mut p = Pacing::new();
    p.set(pace);
    let refresh = 1000.0 / hz;
    let mut t = 0.0;
    for _ in 0..WINDOW {
        p.frame(t);
        t += refresh;
    }
    (p, t, refresh)
}

#[test]
fn every_second_frame_at_120_hz() {
    let (mut p, t, refresh) = at(120.0, PACE);
    assert!(p.admit(t));
    assert!(!p.admit(t + refresh));
    assert!(p.admit(t + 2.0 * refresh));
}

#[test]
fn every_frame_at_60_hz() {
    let (mut p, t, refresh) = at(60.0, PACE);
    assert!(p.admit(t));
    assert!(p.admit(t + refresh));
}

#[test]
fn the_core_asks_a_slower_pace() {
    let (mut p, t, refresh) = at(
        60.0,
        Pace {
            asked_ms: 33.0,
            ..PACE
        },
    );
    assert!(p.admit(t));
    assert!(!p.admit(t + refresh));
    assert!(p.admit(t + 2.0 * refresh));
}

#[test]
fn a_gap_with_a_missed_frame_does_not_move_the_refresh() {
    let (mut p, t, refresh) = at(60.0, PACE);
    p.frame(t + 3.0 * refresh);
    let g = p.grant(t, t);
    assert!((g.refresh_ms - refresh).abs() < 0.01);
}

#[test]
fn settled_turns_take_two_frames() {
    let (p, t, _) = at(60.0, PACE);
    let g = p.grant(t, t);
    assert!(!g.interactive);
    assert!((g.deadline_at - (t + 2.0 * g.paced_ms * 0.9)).abs() < 0.01);
}

#[test]
fn input_makes_turns_interactive_until_they_settle() {
    let (mut p, t, _) = at(60.0, PACE);
    p.input(t);
    let g = p.grant(t, t + 149.0);
    assert!(g.interactive);
    assert!((g.deadline_at - (t + g.paced_ms * 0.9)).abs() < 0.01);
    assert!(!p.grant(t, t + 151.0).interactive);
}

#[test]
fn a_turn_that_wants_the_next_is_interactive() {
    let (mut p, t, _) = at(60.0, PACE);
    p.asked_next(true);
    assert!(p.grant(t, t).interactive);
}
