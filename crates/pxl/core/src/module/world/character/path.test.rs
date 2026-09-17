// pxl_core/module/world/character/path.test.rs

use super::*;

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} is not {b}");
}

fn plan(start: Vec2, target: Vec2, duration: f64) -> WalkPlan {
    WalkPlan {
        start,
        target,
        start_time: 0.0,
        duration,
    }
}

#[test]
fn heading_to_points_along_x() {
    assert_eq!(heading_to(v(0.0, 0.0), v(5.0, 0.0)), 0.0);
}

#[test]
fn heading_to_points_along_y() {
    close(heading_to(v(0.0, 0.0), v(0.0, 5.0)), core::f64::consts::PI / 2.0);
}

#[test]
fn sample_walk_plan_at_start_time_sits_exactly_at_start() {
    let r = sample_walk_plan(&plan(v(0.0, 0.0), v(10.0, 0.0), 1.0), 0.0);
    close(r.x, 0.0);
    close(r.y, 0.0);
    assert!(!r.arrived);
}

#[test]
fn sample_walk_plan_halfway_through_duration_is_halfway_to_target_regardless_of_distance() {
    let near = sample_walk_plan(&plan(v(0.0, 0.0), v(2.0, 0.0), 4.0), 2.0);
    let far = sample_walk_plan(&plan(v(0.0, 0.0), v(2000.0, 0.0), 4.0), 2.0);
    close(near.x, 1.0);
    close(far.x, 1000.0);
    assert!(!near.arrived);
    assert!(!far.arrived);
}

#[test]
fn sample_walk_plan_arrives_exactly_at_target_once_duration_has_elapsed_for_any_distance() {
    let near = sample_walk_plan(&plan(v(0.0, 0.0), v(2.0, 0.0), 4.0), 4.0);
    let far = sample_walk_plan(&plan(v(0.0, 0.0), v(2000.0, 3000.0), 4.0), 4.0);
    close(near.x, 2.0);
    assert!(near.arrived);
    assert_eq!(near.heading, None);
    close(far.x, 2000.0);
    close(far.y, 3000.0);
    assert!(far.arrived);
}

#[test]
fn sample_walk_plan_clamps_past_the_end_instead_of_overshooting() {
    let r = sample_walk_plan(&plan(v(0.0, 0.0), v(5.0, 0.0), 1.0), 100.0);
    close(r.x, 5.0);
    assert!(r.arrived);
}

#[test]
fn sample_walk_plan_heading_points_from_start_toward_target_while_still_en_route() {
    let r = sample_walk_plan(&plan(v(0.0, 0.0), v(0.0, 10.0), 4.0), 1.0);
    close(r.heading.expect("en route"), core::f64::consts::PI / 2.0);
}

#[test]
fn sample_walk_plan_zero_duration_arrives_immediately() {
    let r = sample_walk_plan(&plan(v(0.0, 0.0), v(5.0, 5.0), 0.0), 0.0);
    assert!(r.arrived);
    close(r.x, 5.0);
    close(r.y, 5.0);
}

const SETTLE_DELAY: f64 = 0.15;

#[test]
fn update_settle_tracker_a_target_that_never_moves_never_settles_again() {
    let tracker = init_settle_tracker(v(1.0, 2.0), 0.0);
    assert!(!update_settle_tracker(&tracker, v(1.0, 2.0), 10.0, SETTLE_DELAY).just_settled);
}

#[test]
fn update_settle_tracker_a_continuously_moving_target_mid_drag_never_settles() {
    let mut tracker = init_settle_tracker(v(0.0, 0.0), 0.0);
    let mut t = 0.0;
    let mut fired = false;
    for _ in 0..60 {
        t += 1.0 / 60.0;
        let r = update_settle_tracker(&tracker, v(t * 10.0, 0.0), t, SETTLE_DELAY);
        tracker = r.tracker;
        fired |= r.just_settled;
    }
    assert!(!fired);
}

#[test]
fn update_settle_tracker_fires_once_the_target_stops_changing_for_settle_delay() {
    let tracker = init_settle_tracker(v(0.0, 0.0), 0.0);
    let during = update_settle_tracker(&tracker, v(5.0, 5.0), 1.0, SETTLE_DELAY);
    assert!(!during.just_settled);
    let too_soon = update_settle_tracker(&during.tracker, v(5.0, 5.0), 1.0 + SETTLE_DELAY / 2.0, SETTLE_DELAY);
    assert!(!too_soon.just_settled);
    // Nudged past the boundary: (1 + SETTLE_DELAY) - 1 doesn't round-trip to
    // exactly SETTLE_DELAY in floating point.
    let settled = update_settle_tracker(&too_soon.tracker, v(5.0, 5.0), 1.0 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
    assert!(settled.just_settled);
    assert_eq!(settled.tracker.settled, v(5.0, 5.0));
}

#[test]
fn update_settle_tracker_only_fires_once_per_settle_not_on_every_subsequent_stable_frame() {
    let tracker = init_settle_tracker(v(0.0, 0.0), 0.0);
    let after_change = update_settle_tracker(&tracker, v(5.0, 5.0), 1.0, SETTLE_DELAY);
    let first = update_settle_tracker(&after_change.tracker, v(5.0, 5.0), 1.0 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
    assert!(first.just_settled);
    let second = update_settle_tracker(&first.tracker, v(5.0, 5.0), 1.0 + SETTLE_DELAY + 1.0, SETTLE_DELAY);
    assert!(!second.just_settled);
}

#[test]
fn update_settle_tracker_a_mid_transition_jump_resets_the_settle_clock() {
    let tracker = init_settle_tracker(v(0.0, 0.0), 0.0);
    let step1 = update_settle_tracker(&tracker, v(1.0, 0.0), 0.05, SETTLE_DELAY);
    let step2 = update_settle_tracker(&step1.tracker, v(2.0, 0.0), 0.1, SETTLE_DELAY);
    assert!(!step2.just_settled);
    let not_yet = update_settle_tracker(&step2.tracker, v(2.0, 0.0), 0.1 + SETTLE_DELAY - 0.01, SETTLE_DELAY);
    assert!(!not_yet.just_settled);
    let now_settled = update_settle_tracker(&not_yet.tracker, v(2.0, 0.0), 0.1 + SETTLE_DELAY + 0.01, SETTLE_DELAY);
    assert!(now_settled.just_settled);
}

#[test]
fn update_settle_tracker_still_detects_movement_when_the_caller_reuses_one_target_value() {
    let mut shared = v(0.0, 0.0);
    let tracker = init_settle_tracker(shared, 0.0);
    shared.x = 5.0;
    shared.y = 5.0;
    let during = update_settle_tracker(&tracker, shared, 1.0, SETTLE_DELAY);
    assert!(during.moved);
    assert!(!during.just_settled);
    let settled = update_settle_tracker(&during.tracker, shared, 1.0 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
    assert!(settled.just_settled);
    assert_eq!(settled.tracker.settled, v(5.0, 5.0));
    shared.x = 9.0;
    assert!(update_settle_tracker(&settled.tracker, shared, 1.0 + SETTLE_DELAY + 0.01, SETTLE_DELAY).moved);
}
