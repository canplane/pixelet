// xpute-guest/sched/tick.test.rs

use core::cell::{Cell, RefCell};

use super::*;
use crate::clock;

// A clock the test moves, and what the steps saw, per thread.
thread_local! {
    static CLOCK_MS: Cell<f64> = const { Cell::new(0.0) };
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
}

fn clock() -> f64 {
    CLOCK_MS.with(|c| c.get())
}

fn burn(ms: f64) {
    CLOCK_MS.with(|c| c.set(c.get() + ms));
}

#[derive(Clone, Default)]
struct Seen {
    order: Vec<&'static str>,
    a: f64,
    b: f64,
    runs: u32,
    id: u32,
}

fn seen<R>(f: impl FnOnce(&mut Seen) -> R) -> R {
    SEEN.with(|s| f(&mut s.borrow_mut()))
}

fn saw() -> Seen {
    seen(|s| s.clone())
}

const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: 4.0,
    content_share: 0.2,
};

const QUOTA_MS: f64 = 6.0;

fn tick(start: Seen) -> Tick<8> {
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    seen(|s| *s = start);
    Tick::new(POLICY)
}

#[test]
fn tick_phases_run_in_their_fixed_order_whatever_order_they_registered_in() {
    let mut t = tick(Seen::default());
    t.on_tick(Phase::report, |_| seen(|s| s.order.push("report")), 0);
    t.on_tick(Phase::cosmetic, |_| seen(|s| s.order.push("cosmetic")), 0);
    t.on_tick(Phase::content, |_| seen(|s| s.order.push("content")), 0);
    t.on_tick(Phase::visible, |_| seen(|s| s.order.push("visible")), 0);
    t.on_tick(Phase::interaction, |_| seen(|s| s.order.push("interaction")), 0);
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, PHASES);
}

#[test]
fn tick_within_a_phase_steps_run_in_registration_order() {
    let mut t = tick(Seen::default());
    t.on_tick(Phase::visible, |_| seen(|s| s.order.push("1")), 0);
    t.on_tick(Phase::visible, |_| seen(|s| s.order.push("2")), 0);
    t.on_tick(Phase::visible, |_| seen(|s| s.order.push("3")), 0);
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, ["1", "2", "3"]);
}

#[test]
fn tick_one_budget_is_shared_what_an_earlier_phase_spends_a_later_one_does_not_get() {
    let mut t = tick(Seen { a: -1.0, ..Default::default() });
    t.on_tick(Phase::visible, |c| burn(c.pass.total_ms * 0.75), 0);
    t.on_tick(Phase::content, |c| seen(|s| s.a = c.pass.remaining()), 0);
    let report = t.run(0.016, QUOTA_MS, now());
    let s = saw();
    assert!(s.a >= 0.0);
    assert!(s.a <= report.budget_ms * 0.3, "content saw {} of {}", s.a, report.budget_ms);
}

#[test]
fn tick_the_visible_phase_cannot_take_the_content_share_and_content_sees_it() {
    let mut t = tick(Seen::default());
    t.on_tick(
        Phase::visible,
        |c| {
            seen(|s| s.a = c.pass.remaining());
            c.pass.run(
                0..1000,
                |_| {
                    burn(0.05);
                    true
                },
                None,
            );
        },
        0,
    );
    t.on_tick(Phase::content, |c| seen(|s| s.b = c.pass.remaining()), 0);
    let report = t.run(0.016, QUOTA_MS, now());
    let s = saw();
    assert!(s.a <= report.budget_ms * 0.8 + 0.01, "visible saw {} of {}", s.a, report.budget_ms);
    assert!(s.b >= report.budget_ms * 0.1, "content saw {} of {}", s.b, report.budget_ms);
}

#[test]
fn tick_a_step_that_unregisters_itself_mid_tick_is_not_run_again() {
    let mut t = tick(Seen::default());
    let id = t.on_tick(
        Phase::cosmetic,
        |c| {
            let id = seen(|s| {
                s.runs += 1;
                s.id
            });
            c.off_tick(id);
        },
        0,
    );
    seen(|s| s.id = id);
    t.run(0.016, QUOTA_MS, now());
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().runs, 1);
}

#[test]
fn tick_a_step_that_unregisters_an_earlier_one_does_not_make_the_tick_skip_the_step_after_it() {
    let mut t = tick(Seen::default());
    let first = t.on_tick(Phase::visible, |_| seen(|s| s.order.push("first")), 0);
    t.on_tick(
        Phase::visible,
        |c| {
            let id = seen(|s| {
                s.order.push("second");
                s.id
            });
            c.off_tick(id);
        },
        0,
    );
    t.on_tick(Phase::visible, |_| seen(|s| s.order.push("third")), 0);
    seen(|s| s.id = first);
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, ["first", "second", "third"]);
}

#[test]
fn tick_the_report_phase_sees_every_phase_before_it() {
    let mut t = tick(Seen { a: -1.0, ..Default::default() });
    t.on_tick(Phase::visible, |_| burn(2.0), 0);
    t.on_tick(
        Phase::report,
        |c| {
            seen(|s| {
                s.a = c.phase_ms[Phase::visible as usize];
                s.b = c.phase_ms[Phase::report as usize];
            })
        },
        0,
    );
    t.run(0.016, QUOTA_MS, now());
    let s = saw();
    assert!(s.a >= 2.0, "visible {}", s.a);
    assert_eq!(s.b, 0.0);
}

const COST: f64 = 0.3;

#[test]
fn tick_a_storm_of_steps_that_cost_time_and_report_none_stays_within_the_hard_ceiling() {
    let mut t = tick(Seen::default());
    t.on_tick(
        Phase::visible,
        |c| {
            c.pass.run(
                0..2000,
                |_| {
                    burn(COST);
                    false
                },
                None,
            );
        },
        0,
    );
    let report = t.run(0.016, QUOTA_MS, now());
    assert!(
        report.phase_ms[Phase::visible as usize] <= report.budget_ms + POLICY.hard_slack_ms + COST,
        "visible {} over budget {}",
        report.phase_ms[Phase::visible as usize],
        report.budget_ms
    );
}

#[test]
fn tick_what_the_turn_spent_before_the_tick_comes_off_the_budget() {
    let mut t = tick(Seen::default());
    t.on_tick(Phase::visible, |c| seen(|s| s.a = c.pass.remaining()), 0);
    let edge = now();
    burn(2.0);
    t.run(1.0 / 60.0, QUOTA_MS, edge);
    let a = saw().a;
    assert!(a <= QUOTA_MS * 0.8 - 2.0 + 0.01, "visible saw {a}");
}

#[test]
fn tick_what_the_interaction_phase_spends_comes_off_the_budget() {
    let mut t = tick(Seen::default());
    t.on_tick(Phase::interaction, |_| burn(1.5), 0);
    t.on_tick(
        Phase::visible,
        |c| {
            seen(|s| {
                s.a = c.pass.remaining();
                s.b = c.pass.total_ms;
            })
        },
        0,
    );
    t.run(1.0 / 60.0, QUOTA_MS, now());
    let s = saw();
    assert!(s.a <= s.b - 1.5, "visible saw {} of {}", s.a, s.b);
}
