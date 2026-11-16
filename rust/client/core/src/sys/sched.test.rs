// pxl-core/sys/sched.test.rs

//! The entry's deadline (`Budget`), at a 60 Hz target: a frame's budget is
//! its interval less `MARGIN_SHARE`, and the content's share `CONTENT_SHARE`
//! of that.

use super::*;

const TARGET_MS: f64 = 1000.0 / 60.0;
const FRAME_MS: f64 = TARGET_MS * (1.0 - MARGIN_SHARE);
const CONTENT_MS: f64 = CONTENT_SHARE * FRAME_MS;

/// Before any vsync, an entry gets the content's share from its start.
#[test]
fn an_entry_before_any_frame_gets_the_content_share() {
    let mut b = Budget::NONE;
    b.begin(100.0, TARGET_MS);
    assert_eq!(b.end(), 100.0 + CONTENT_MS);
}

/// A frame's entry ends at the frame's budget from the frame's start, the
/// start the display gave, though the entry began later.
#[test]
fn a_frame_ends_at_its_budget_from_its_start() {
    let mut b = Budget::NONE;
    b.begin(101.0, TARGET_MS);
    b.begin_frame(100.0, TARGET_MS);
    assert_eq!(b.end(), 100.0 + FRAME_MS);
}

/// An entry with no frame gets what the last frame's budget leaves, and at
/// least the content's share from its own start.
#[test]
fn an_entry_without_a_frame_gets_what_the_frame_leaves() {
    let mut b = Budget::NONE;
    b.begin(100.0, TARGET_MS);
    b.begin_frame(100.0, TARGET_MS);
    b.begin(103.0, TARGET_MS);
    assert_eq!(b.end(), 100.0 + FRAME_MS, "within the frame's budget");
    let late = 100.0 + FRAME_MS;
    b.begin(late, TARGET_MS);
    assert_eq!(b.end(), late + CONTENT_MS, "past it");
}

/// The visible phase ends the content's share sooner, and gives it back.
#[test]
fn the_visible_phase_leaves_the_content_share() {
    let mut b = Budget::NONE;
    b.begin(100.0, TARGET_MS);
    b.begin_frame(100.0, TARGET_MS);
    b.reserved = true;
    assert_eq!(b.end(), 100.0 + FRAME_MS - CONTENT_MS);
    b.reserved = false;
    assert_eq!(b.end(), 100.0 + FRAME_MS);
}

/// A frame that ran past its deadline still leaves the entry the content's
/// share from its end; one that ended early leaves the frame's budget.
#[test]
fn a_frame_s_end_leaves_the_content_share() {
    let mut b = Budget::NONE;
    b.begin(100.0, TARGET_MS);
    b.begin_frame(100.0, TARGET_MS);
    let over = 100.0 + FRAME_MS + 4.0;
    b.deadline_from(over);
    assert_eq!(b.end(), over + CONTENT_MS, "overran");
    b.begin_frame(200.0, TARGET_MS);
    b.deadline_from(205.0);
    assert_eq!(b.end(), 200.0 + FRAME_MS, "in time");
}
