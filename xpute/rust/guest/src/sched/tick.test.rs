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

// A program's phases, as a world's might number them.
#[allow(non_upper_case_globals, reason = "named as a program's phases are")]
mod phase {
    use super::Phase;
    pub const interaction: Phase = 0;
    pub const visible: Phase = 1;
    pub const content: Phase = 2;
    pub const cosmetic: Phase = 3;
    pub const report: Phase = 4;
}

const PHASES: [&str; 5] = ["interaction", "visible", "content", "cosmetic", "report"];

/// What the visible phase holds back for content.
const CONTENT_SHARE: f64 = 0.2;

const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: 4.0,
    held_back: &[0.0, CONTENT_SHARE, 0.0, 0.0, 0.0],
};

const QUOTA_MS: f64 = 6.0;

fn tick(start: Seen) -> Tick<8> {
    clock::set(clock);
    CLOCK_MS.with(|c| c.set(0.0));
    seen(|s| *s = start);
    Tick::new(POLICY)
}

#[test]
fn phase_order() {
    let mut t = tick(Seen::default());
    t.on_tick(phase::report, |_| seen(|s| s.order.push("report")), 0);
    t.on_tick(phase::cosmetic, |_| seen(|s| s.order.push("cosmetic")), 0);
    t.on_tick(phase::content, |_| seen(|s| s.order.push("content")), 0);
    t.on_tick(phase::visible, |_| seen(|s| s.order.push("visible")), 0);
    t.on_tick(
        phase::interaction,
        |_| seen(|s| s.order.push("interaction")),
        0,
    );
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, PHASES);
}

#[test]
fn step_order() {
    let mut t = tick(Seen::default());
    t.on_tick(phase::visible, |_| seen(|s| s.order.push("1")), 0);
    t.on_tick(phase::visible, |_| seen(|s| s.order.push("2")), 0);
    t.on_tick(phase::visible, |_| seen(|s| s.order.push("3")), 0);
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, ["1", "2", "3"]);
}

#[test]
fn shared_budget() {
    let mut t = tick(Seen {
        a: -1.0,
        ..Default::default()
    });
    t.on_tick(phase::visible, |c| burn(c.pass.total_ms * 0.75), 0);
    t.on_tick(phase::content, |c| seen(|s| s.a = c.pass.remaining()), 0);
    let report = t.run(0.016, QUOTA_MS, now());
    let s = saw();
    assert!(s.a >= 0.0);
    assert!(
        s.a <= report.budget_ms * 0.3,
        "content saw {} of {}",
        s.a,
        report.budget_ms
    );
}

#[test]
fn held_back_kept_for_later_phases() {
    let mut t = tick(Seen::default());
    t.on_tick(
        phase::visible,
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
    t.on_tick(phase::content, |c| seen(|s| s.b = c.pass.remaining()), 0);
    let report = t.run(0.016, QUOTA_MS, now());
    let s = saw();
    assert!(
        s.a <= report.budget_ms * 0.8 + 0.01,
        "visible saw {} of {}",
        s.a,
        report.budget_ms
    );
    assert!(
        s.b >= report.budget_ms * 0.1,
        "content saw {} of {}",
        s.b,
        report.budget_ms
    );
}

#[test]
fn self_unregister() {
    let mut t = tick(Seen::default());
    let id = t.on_tick(
        phase::cosmetic,
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
fn unregister_earlier_no_skip() {
    let mut t = tick(Seen::default());
    let first = t.on_tick(phase::visible, |_| seen(|s| s.order.push("first")), 0);
    t.on_tick(
        phase::visible,
        |c| {
            let id = seen(|s| {
                s.order.push("second");
                s.id
            });
            c.off_tick(id);
        },
        0,
    );
    t.on_tick(phase::visible, |_| seen(|s| s.order.push("third")), 0);
    seen(|s| s.id = first);
    t.run(0.016, QUOTA_MS, now());
    assert_eq!(saw().order, ["first", "second", "third"]);
}

#[test]
fn report_sees_all() {
    let mut t = tick(Seen {
        a: -1.0,
        ..Default::default()
    });
    t.on_tick(phase::visible, |_| burn(2.0), 0);
    t.on_tick(
        phase::report,
        |c| {
            seen(|s| {
                s.a = c.phase_ms[phase::visible as usize];
                s.b = c.phase_ms[phase::report as usize];
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
fn silent_storm_capped() {
    let mut t = tick(Seen::default());
    t.on_tick(
        phase::visible,
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
        report.phase_ms[phase::visible as usize] <= report.budget_ms + POLICY.hard_slack_ms + COST,
        "visible {} over budget {}",
        report.phase_ms[phase::visible as usize],
        report.budget_ms
    );
}

#[test]
fn prior_spend_deducted() {
    let mut t = tick(Seen::default());
    t.on_tick(phase::visible, |c| seen(|s| s.a = c.pass.remaining()), 0);
    let edge = now();
    burn(2.0);
    t.run(1.0 / 60.0, QUOTA_MS, edge);
    let a = saw().a;
    assert!(a <= QUOTA_MS * 0.8 - 2.0 + 0.01, "visible saw {a}");
}

#[test]
fn interaction_spend_deducted() {
    let mut t = tick(Seen::default());
    t.on_tick(phase::interaction, |_| burn(1.5), 0);
    t.on_tick(
        phase::visible,
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
