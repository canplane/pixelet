// pxl-core/sys/sched.test.rs

//! The entry's end (`Budget`), at a 60 Hz target: the margin is
//! `MARGIN_SHARE` of the interval, a frame's budget what it leaves, and a
//! slice `SLICE_SHARE` of that.

use core::task::Context;

use super::*;
use crate::sys::host::set_clock;

const TARGET_MS: f64 = 1000.0 / 60.0;
const MARGIN_MS: f64 = MARGIN_SHARE * TARGET_MS;
const FRAME_MS: f64 = TARGET_MS - MARGIN_MS;
const SLICE_MS: f64 = SLICE_SHARE * FRAME_MS;
const DISPLAY_MS: f64 = TARGET_MS / 2.0;

/// A budget paced at the target, its last frame started at `frame_at`.
fn paced(frame_at: f64) -> Budget {
    Budget {
        frame_at,
        display_ms: DISPLAY_MS,
        target_ms: TARGET_MS,
        ..Budget::NONE
    }
}

/// With no frame due, an entry ends one slice from its own start, wherever
/// the last frame started.
#[test]
fn an_entry_with_no_frame_due_gets_a_slice() {
    let mut b = paced(100.0);
    b.begin(103.0);
    assert_eq!(b.end(false), 103.0 + SLICE_MS);
    b.begin(500.0);
    assert_eq!(b.end(false), 500.0 + SLICE_MS);
}

/// While a frame is due one target after the last frame's start, every entry
/// ends the margin before it, and an entry that starts from there until half a
/// display interval past it ends at its start.
#[test]
fn an_entry_ends_the_margin_before_the_frame_due() {
    let mut b = paced(100.0);
    b.begin(103.0);
    assert_eq!(b.end(true), 100.0 + FRAME_MS);
    b.begin(100.0 + FRAME_MS - 0.5);
    assert_eq!(b.end(true), 100.0 + FRAME_MS, "no slice of its own");
    for at in [100.0 + FRAME_MS + 0.5, 100.0 + TARGET_MS + 0.5] {
        b.begin(at);
        assert_eq!(
            b.end(true),
            at,
            "past its end, the frame's start still known"
        );
    }
}

/// A frame due whose start the core cannot place leaves the entry its slice:
/// half a display interval past one target from the last frame's start, as after a still
/// view, an overrun or on a hidden page, and before any frame.
#[test]
fn a_frame_due_at_no_known_start_leaves_a_slice() {
    let mut b = paced(100.0);
    for at in [100.0 + TARGET_MS + DISPLAY_MS / 2.0, 5000.0] {
        b.begin(at);
        assert_eq!(b.end(true), at + SLICE_MS);
    }
    let mut b = paced(f64::NEG_INFINITY);
    b.begin(100.0);
    assert_eq!(b.end(true), 100.0 + SLICE_MS, "before any frame");
}

/// A frame's entry ends at the frame's budget from the frame's start, the
/// start the display gave, though the entry began later.
#[test]
fn a_frame_ends_at_its_budget_from_its_start() {
    let mut b = paced(0.0);
    b.begin(101.0);
    b.frame_at = 100.0;
    assert_eq!(b.end(true), 100.0 + FRAME_MS);
}

/// The frame task's ask for a frame, which this takes.
fn take_invalid() -> bool {
    let mut wait = pin!(invalidated());
    wait.as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready()
}

/// Starts an entry at `at`.
fn enter(at: f64) {
    set_clock(at);
    begin();
}

/// A frame is due from its ask until it has run: an entry has its slice
/// before the ask, the frame's budget while the ask is out and while the
/// frame runs, no budget once the frame has run, and after that the margin
/// before the next frame where the frame asked for it, else its slice.
#[test]
fn a_frame_is_due_from_its_ask_until_it_has_run() {
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    enter(1000.0);
    assert!(
        near(budget_ms(), 0.0),
        "no target before a frame is asked for"
    );
    invalidate(InvalidationReason::Pending);
    assert!(take_invalid());
    set_pace_ms((DISPLAY_MS, TARGET_MS));
    enter(1001.0);
    assert!(near(budget_ms(), SLICE_MS), "due before any frame");

    // The frame starts at 1010, in an entry begun at 1011, and asks for the
    // next as it runs.
    enter(1011.0);
    begin_frame(1010.0);
    assert!(near(budget_ms(), FRAME_MS - 1.0), "the frame's own entry");
    invalidate(InvalidationReason::Scene);
    assert!(!spent());
    end_frame();
    assert!(spent(), "the entry that drew");
    assert_eq!(budget_ms(), 0.0);
    enter(1013.0);
    assert!(
        near(budget_ms(), FRAME_MS - 3.0),
        "asked, and not yet taken"
    );
    assert!(take_invalid());
    set_pace_ms((DISPLAY_MS, TARGET_MS));
    assert!(near(budget_ms(), FRAME_MS - 3.0), "taken, and not yet run");
    enter(1010.0 + TARGET_MS);
    assert_eq!(budget_ms(), 0.0, "within the margin of the frame's start");
    enter(3000.0);
    assert!(near(budget_ms(), SLICE_MS), "the vsync never answered");

    // The next frame asks for none.
    enter(3011.0);
    begin_frame(3010.0);
    end_frame();
    enter(3013.0);
    assert!(near(budget_ms(), SLICE_MS), "no frame due");
    assert!(!take_invalid());
}
