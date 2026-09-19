// xpute-runtime/sched/tick.test.rs
//
// A step is a plain fn, so what a step sees goes in the state it runs over.

use std::sync::OnceLock;
use std::time::Instant;

use super::*;
use crate::clock::set_clock;

fn wall_clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

fn burn(ms: f64) {
    let end = now() + ms;
    while now() < end {}
}

#[derive(Default)]
struct Seen {
    order: Vec<&'static str>,
    a: f64,
    b: f64,
    runs: u32,
    id: u32,
}

const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: 4.0,
    content_share: 0.2,
};

const QUOTA_MS: f64 = 6.0;

fn tick() -> Tick<Seen, 8> {
    set_clock(wall_clock);
    Tick::new(POLICY)
}

#[test]
fn tick_phases_run_in_their_fixed_order_whatever_order_they_registered_in() {
    let mut t = tick();
    t.on_tick(REPORT, |c| c.state.order.push("report"), "t");
    t.on_tick(COSMETIC, |c| c.state.order.push("cosmetic"), "t");
    t.on_tick(CONTENT, |c| c.state.order.push("content"), "t");
    t.on_tick(VISIBLE, |c| c.state.order.push("visible"), "t");
    t.on_tick(INTERACTION, |c| c.state.order.push("interaction"), "t");
    let mut s = Seen::default();
    t.run(&mut s, 0.016, QUOTA_MS, now());
    assert_eq!(s.order, PHASES);
}

#[test]
fn tick_within_a_phase_steps_run_in_registration_order() {
    let mut t = tick();
    t.on_tick(VISIBLE, |c| c.state.order.push("1"), "t");
    t.on_tick(VISIBLE, |c| c.state.order.push("2"), "t");
    t.on_tick(VISIBLE, |c| c.state.order.push("3"), "t");
    let mut s = Seen::default();
    t.run(&mut s, 0.016, QUOTA_MS, now());
    assert_eq!(s.order, ["1", "2", "3"]);
}

#[test]
fn tick_one_budget_is_shared_what_an_earlier_phase_spends_a_later_one_does_not_get() {
    let mut t = tick();
    t.on_tick(VISIBLE, |c| burn(c.budget.total_ms * 0.75), "t");
    t.on_tick(CONTENT, |c| c.state.a = c.budget.remaining(), "t");
    let mut s = Seen { a: -1.0, ..Default::default() };
    let report = t.run(&mut s, 0.016, QUOTA_MS, now());
    assert!(s.a >= 0.0);
    assert!(s.a <= report.budget_ms * 0.3, "content saw {} of {}", s.a, report.budget_ms);
}

#[test]
fn tick_the_visible_phase_cannot_take_the_content_share_and_content_sees_it() {
    let mut t = tick();
    t.on_tick(
        VISIBLE,
        |c| {
            c.state.a = c.budget.remaining();
            // Runs the budget it was shown to the floor.
            c.budget.run(
                0..1000,
                |_| {
                    burn(0.05);
                    true
                },
                None,
            );
        },
        "t",
    );
    t.on_tick(CONTENT, |c| c.state.b = c.budget.remaining(), "t");
    let mut s = Seen::default();
    let report = t.run(&mut s, 0.016, QUOTA_MS, now());
    assert!(s.a <= report.budget_ms * 0.8 + 0.01, "visible saw {} of {}", s.a, report.budget_ms);
    assert!(s.b >= report.budget_ms * 0.1, "content saw {} of {}", s.b, report.budget_ms);
}

#[test]
fn tick_a_step_that_unregisters_itself_mid_tick_is_not_run_again() {
    let mut t = tick();
    let id = t.on_tick(
        COSMETIC,
        |c| {
            c.state.runs += 1;
            let id = c.state.id;
            c.off_tick(id);
        },
        "t",
    );
    let mut s = Seen { id, ..Default::default() };
    t.run(&mut s, 0.016, QUOTA_MS, now());
    t.run(&mut s, 0.016, QUOTA_MS, now());
    assert_eq!(s.runs, 1);
}

#[test]
fn tick_a_step_that_unregisters_an_earlier_one_does_not_make_the_tick_skip_the_step_after_it() {
    let mut t = tick();
    let first = t.on_tick(VISIBLE, |c| c.state.order.push("first"), "t");
    t.on_tick(
        VISIBLE,
        |c| {
            c.state.order.push("second");
            let id = c.state.id;
            c.off_tick(id);
        },
        "t",
    );
    t.on_tick(VISIBLE, |c| c.state.order.push("third"), "t");
    let mut s = Seen { id: first, ..Default::default() };
    t.run(&mut s, 0.016, QUOTA_MS, now());
    assert_eq!(s.order, ["first", "second", "third"]);
}

#[test]
fn tick_the_report_phase_sees_every_phase_before_it() {
    let mut t = tick();
    t.on_tick(VISIBLE, |_| burn(2.0), "t");
    t.on_tick(
        REPORT,
        |c| {
            c.state.a = c.phase_ms[VISIBLE];
            c.state.b = c.phase_ms[REPORT];
        },
        "t",
    );
    let mut s = Seen { a: -1.0, ..Default::default() };
    t.run(&mut s, 0.016, QUOTA_MS, now());
    assert!(s.a >= 2.0, "visible {}", s.a);
    assert_eq!(s.b, 0.0);
}

#[test]
fn frame_budget_run_forward_progress_survives_an_exhausted_pool() {
    set_clock(wall_clock);
    let budget = FrameBudget::new(0.0, now(), POLICY.hard_slack_ms);
    burn(1.0);
    let stats = budget.run([1, 2, 3], |_| true, None);
    assert_eq!(stats.ran, 1);
    assert!(stats.stopped_early);
}

const COST: f64 = 0.3;

#[test]
fn tick_a_storm_of_steps_that_report_work_stays_within_the_budget_plus_one_step() {
    let mut t = tick();
    t.on_tick(
        VISIBLE,
        |c| {
            c.state.runs = c
                .budget
                .run(
                    0..2000,
                    |_| {
                        burn(COST);
                        true
                    },
                    None,
                )
                .ran;
        },
        "t",
    );
    let mut s = Seen::default();
    let report = t.run(&mut s, 0.016, QUOTA_MS, now());
    assert!(s.runs >= 1, "forward progress");
    assert!(s.runs < 2000, "the storm was cut short");
    assert!(
        report.phase_ms[VISIBLE] <= report.budget_ms + COST + 1.0,
        "visible {} over budget {}",
        report.phase_ms[VISIBLE],
        report.budget_ms
    );
}

#[test]
fn tick_a_storm_of_steps_that_cost_time_and_report_none_stays_within_the_hard_ceiling() {
    let mut t = tick();
    t.on_tick(
        VISIBLE,
        |c| {
            c.budget.run(
                0..2000,
                |_| {
                    burn(COST);
                    false
                },
                None,
            );
        },
        "t",
    );
    let mut s = Seen::default();
    let report = t.run(&mut s, 0.016, QUOTA_MS, now());
    assert!(
        report.phase_ms[VISIBLE] <= report.budget_ms + POLICY.hard_slack_ms + COST + 1.0,
        "visible {} over budget {}",
        report.phase_ms[VISIBLE],
        report.budget_ms
    );
}

#[test]
fn tick_what_the_turn_spent_before_the_tick_comes_off_the_budget() {
    let mut t = tick();
    t.on_tick(VISIBLE, |c| c.state.a = c.budget.remaining(), "t");
    let edge = now();
    burn(2.0);
    let mut s = Seen::default();
    t.run(&mut s, 1.0 / 60.0, QUOTA_MS, edge);
    assert!(s.a <= QUOTA_MS * 0.8 - 2.0 + 0.01, "visible saw {}", s.a);
}

#[test]
fn tick_what_the_interaction_phase_spends_comes_off_the_budget() {
    let mut t = tick();
    t.on_tick(
        INTERACTION,
        |_| {
            burn(1.5);
        },
        "t",
    );
    t.on_tick(
        VISIBLE,
        |c| {
            c.state.a = c.budget.remaining();
            c.state.b = c.budget.total_ms;
        },
        "t",
    );
    let mut s = Seen::default();
    t.run(&mut s, 1.0 / 60.0, QUOTA_MS, now());
    assert!(s.a <= s.b - 1.5, "visible saw {} of {}", s.a, s.b);
}

#[test]
#[should_panic(expected = "steps are taken")]
fn tick_the_step_table_is_fixed_a_step_past_it_is_refused() {
    let mut t = tick();
    for _ in 0..9 {
        t.on_tick(COSMETIC, |_| {}, "t");
    }
}
