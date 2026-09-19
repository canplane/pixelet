// xpute-runtime/sched/frame_budget.test.rs

use std::sync::OnceLock;
use std::time::Instant;

use super::*;
use crate::clock::set_clock;

/// The budget is measured with the host's clock, so a step has to actually
/// take time for the deadline to mean anything: a real one for these tests.
/// The clock is the thread's, so tests on other threads do not see it.
fn wall_clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

fn burn(ms: f64) {
    let end = now() + ms;
    while now() < end {}
}

fn opts(budget_ms: f64) -> BudgetOpts {
    set_clock(wall_clock);
    BudgetOpts { budget_ms, ..Default::default() }
}

#[test]
fn run_under_budget_runs_everything_when_the_budget_is_ample() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3],
        |n| {
            seen.push(n);
            true
        },
        opts(1000.0),
    );
    assert_eq!(seen, [1, 2, 3]);
    assert_eq!(stats.ran, 3);
    assert!(!stats.stopped_early);
}

#[test]
fn run_under_budget_stops_at_the_wall_time_budget() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3, 4, 5],
        |n| {
            seen.push(n);
            burn(6.0);
            true
        },
        opts(10.0),
    );
    // one step fits inside 10ms, the second crosses it, the third is refused
    assert!(seen.len() < 5);
    assert!(stats.stopped_early);
    assert_eq!(stats.ran as usize, seen.len());
}

#[test]
fn run_under_budget_the_first_working_step_always_runs_even_past_budget() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3],
        |n| {
            seen.push(n);
            burn(5.0);
            true
        },
        opts(0.0),
    );
    assert_eq!(seen, [1]);
    assert_eq!(stats.ran, 1);
    assert!(stats.stopped_early);
}

#[test]
fn run_under_budget_max_steps_caps_completed_work() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3, 4, 5],
        |n| {
            seen.push(n);
            true
        },
        BudgetOpts { max_steps: Some(2), ..opts(1000.0) },
    );
    assert_eq!(seen, [1, 2]);
    assert_eq!(stats.ran, 2);
    assert!(stats.stopped_early);
}

#[test]
fn run_under_budget_a_no_op_step_neither_counts_nor_arms_the_guard() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3, 4],
        |n| {
            seen.push(n);
            n == 4
        },
        BudgetOpts { max_steps: Some(1), ..opts(0.0) },
    );
    assert_eq!(seen, [1, 2, 3, 4]);
    assert_eq!(stats.ran, 1);
    assert_eq!(stats.visited, 4);
    assert!(!stats.stopped_early);
}

#[test]
fn run_under_budget_hard_budget_ms_bounds_a_pass_of_expensive_no_ops() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        0..40,
        |n| {
            seen.push(n);
            burn(3.0);
            false
        },
        BudgetOpts {
            hard_budget_ms: Some(12.0),
            ..opts(4.0)
        },
    );
    assert_eq!(stats.ran, 0);
    assert!(stats.stopped_early);
    assert!(seen.len() < 40);
}

#[test]
fn run_under_budget_without_a_ceiling_expensive_no_ops_still_walk_the_whole_list() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        0..6,
        |n| {
            seen.push(n);
            burn(3.0);
            false
        },
        opts(1.0),
    );
    assert_eq!(seen.len(), 6);
    assert!(!stats.stopped_early);
}

#[test]
fn run_under_budget_the_first_item_is_attempted_even_under_a_zero_ceiling() {
    let mut seen = Vec::new();
    let stats = run_under_budget(
        [1, 2, 3],
        |n| {
            seen.push(n);
            burn(2.0);
            true
        },
        BudgetOpts {
            hard_budget_ms: Some(0.0),
            ..opts(0.0)
        },
    );
    assert_eq!(seen, [1]);
    assert_eq!(stats.ran, 1);
}

#[test]
fn run_under_budget_an_empty_list_is_a_clean_no_op() {
    let stats = run_under_budget(core::iter::empty::<u32>(), |_| true, opts(10.0));
    assert_eq!(stats.ran, 0);
    assert_eq!(stats.visited, 0);
    assert!(!stats.stopped_early);
}
