// xpute-guest/sched/pass.test.rs

use core::cell::Cell;

use super::*;
use crate::clock;

// A clock the test moves, per thread.
thread_local! {
    static CLOCK_MS: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    CLOCK_MS.with(|c| c.get())
}

fn burn(ms: f64) {
    CLOCK_MS.with(|c| c.set(c.get() + ms));
}

/// `budget_ms` left from now, and `hard_budget_ms` never waived.
fn pass(budget_ms: f64, hard_budget_ms: Option<f64>) -> Pass {
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    Pass::new(budget_ms, 0.0, hard_budget_ms.map_or(f64::INFINITY, |h| h - budget_ms))
}

#[test]
fn runs_everything_when_the_budget_is_ample() {
    let mut seen = Vec::new();
    let stats = pass(1000.0, None).run(
        [1, 2, 3],
        |n| {
            seen.push(n);
            true
        },
        None,
    );
    assert_eq!(seen, [1, 2, 3]);
    assert_eq!(stats.ran, 3);
    assert!(!stats.stopped_early);
}

#[test]
fn stops_once_the_time_budget_is_spent() {
    let mut seen = Vec::new();
    let stats = pass(10.0, None).run(
        [1, 2, 3, 4, 5],
        |n| {
            seen.push(n);
            burn(6.0);
            true
        },
        None,
    );
    assert_eq!(seen, [1, 2]);
    assert!(stats.stopped_early);
    assert_eq!(stats.ran as usize, seen.len());
}

#[test]
fn the_first_working_step_always_runs_even_past_budget() {
    let mut seen = Vec::new();
    let stats = pass(0.0, None).run(
        [1, 2, 3],
        |n| {
            seen.push(n);
            burn(5.0);
            true
        },
        None,
    );
    assert_eq!(seen, [1]);
    assert_eq!(stats.ran, 1);
    assert!(stats.stopped_early);
}

#[test]
fn max_steps_caps_completed_work() {
    let mut seen = Vec::new();
    let stats = pass(1000.0, None).run(
        [1, 2, 3, 4, 5],
        |n| {
            seen.push(n);
            true
        },
        Some(2),
    );
    assert_eq!(seen, [1, 2]);
    assert_eq!(stats.ran, 2);
    assert!(stats.stopped_early);
}

#[test]
fn a_no_op_step_neither_counts_nor_arms_the_guard() {
    let mut seen = Vec::new();
    let stats = pass(0.0, None).run(
        [1, 2, 3, 4],
        |n| {
            seen.push(n);
            n == 4
        },
        Some(1),
    );
    assert_eq!(seen, [1, 2, 3, 4]);
    assert_eq!(stats.ran, 1);
    assert_eq!(stats.visited, 4);
    assert!(!stats.stopped_early);
}

#[test]
fn the_hard_budget_bounds_a_pass_of_expensive_no_ops() {
    let mut seen = Vec::new();
    let stats = pass(4.0, Some(12.0)).run(
        0..40,
        |n| {
            seen.push(n);
            burn(3.0);
            false
        },
        None,
    );
    assert_eq!(stats.ran, 0);
    assert!(stats.stopped_early);
    // Only the ceiling stops it: four at 3 ms reach 12.
    assert_eq!(seen.len(), 4);
}

#[test]
fn without_a_ceiling_expensive_no_ops_still_walk_the_whole_list() {
    let mut seen = Vec::new();
    let stats = pass(1.0, None).run(
        0..6,
        |n| {
            seen.push(n);
            burn(3.0);
            false
        },
        None,
    );
    assert_eq!(seen.len(), 6);
    assert!(!stats.stopped_early);
}

#[test]
fn the_first_item_is_attempted_even_under_a_zero_ceiling() {
    let mut seen = Vec::new();
    let stats = pass(0.0, Some(0.0)).run(
        [1, 2, 3],
        |n| {
            seen.push(n);
            burn(2.0);
            true
        },
        None,
    );
    assert_eq!(seen, [1]);
    assert_eq!(stats.ran, 1);
}
