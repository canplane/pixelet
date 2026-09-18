// xpute-runtime/ipc/change.test.rs

use core::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ipc::change_waker::ChangeWaker;

const PAGE: Topic = 0;
const CELL: Topic = 1;

fn log(topics: u32, capacity: u32) -> ChangeLog {
    ChangeLog::new(ChangeLogOptions {
        topics,
        capacity,
        keys_per_topic: None,
    })
}

/// A reader's journal loop, the way a caller writes one: interest is a
/// comparison over the span.
fn read(log: &ChangeLog, cursor: Cursor, topic: Topic) -> Option<(Vec<i64>, Vec<u32>, Cursor)> {
    if !log.readable(cursor) {
        return None;
    }
    let (mut keys, mut revisions) = (Vec::new(), Vec::new());
    for p in cursor..log.head() {
        let i = log.slot(p) as usize;
        if log.topic[i] as u32 != topic {
            continue;
        }
        keys.push(log.key[i]);
        revisions.push(log.revision[i]);
    }
    Some((keys, revisions, log.head()))
}

#[test]
fn change_a_bump_advances_the_topic_s_tick_and_becomes_the_key_s_revision() {
    let mut log = log(2, 16);
    assert_eq!(log.revision_of(PAGE, 7), 0);
    assert_eq!(log.bump(PAGE, 7), 1);
    assert_eq!(log.bump(PAGE, 9), 2);
    assert_eq!(log.bump(PAGE, 7), 3);
    assert_eq!(log.revision_of(PAGE, 7), 3);
    assert_eq!(log.revision_of(PAGE, 9), 2);
    assert_eq!(log.tick(PAGE), 3);
    assert_eq!(log.tick(CELL), 0, "topics count separately");
}

#[test]
fn change_a_key_forgotten_and_changed_again_never_repeats_a_revision_a_reader_may_hold() {
    let mut log = log(1, 16);
    log.bump(PAGE, 1);
    let seen = log.revision_of(PAGE, 1);
    log.forget(PAGE, 1);
    assert_eq!(log.revision_of(PAGE, 1), 0);
    log.bump(PAGE, 1);
    assert!(log.revision_of(PAGE, 1) != seen);
}

#[test]
fn change_forgetting_moves_the_last_key_into_the_hole_and_keeps_every_other_revision() {
    let mut log = log(1, 16);
    for k in [10, 20, 30, 40] {
        log.bump(PAGE, k);
    }
    log.forget(PAGE, 20);
    assert_eq!([10, 20, 30, 40].map(|k| log.revision_of(PAGE, k)), [1, 0, 3, 4]);
    log.forget(PAGE, 99);
    assert_eq!(log.revision_of(PAGE, 40), 4, "forgetting an unknown key changes nothing");
}

#[test]
fn change_a_journal_read_returns_the_reader_s_topic_from_its_cursor_and_the_head_to_resume_from() {
    let mut log = log(2, 16);
    let start = log.head();
    log.bump(PAGE, 1);
    log.bump(CELL, 5);
    log.bump(PAGE, 2);
    let first = read(&log, start, PAGE).unwrap();
    assert_eq!(first.0, [1, 2]);
    assert_eq!(first.1, [1, 2]);
    log.bump(PAGE, 1);
    let second = read(&log, first.2, PAGE).unwrap();
    assert_eq!(second.0, [1]);
    assert_eq!(second.1, [3]);
}

#[test]
fn change_a_cursor_the_ring_has_overwritten_is_lost_one_exactly_capacity_behind_is_not() {
    let mut log = log(1, 4);
    let start = log.head();
    for k in 0..4 {
        log.bump(PAGE, k);
    }
    assert!(log.readable(start));
    let whole = read(&log, start, PAGE).unwrap();
    assert_eq!(whole.0, [0, 1, 2, 3]);
    log.bump(PAGE, 4);
    assert!(read(&log, start, PAGE).is_none());
    assert!(log.readable(log.head() - 4));
    assert_eq!(log.revision_of(PAGE, 0), 1, "point reads are unaffected by the ring");
}

#[test]
fn change_a_key_past_2_63_is_the_same_key_in_the_table_and_the_journal() {
    let mut log = log(1, 4);
    let big = ((1u64 << 63) + 5) as i64;
    log.bump(PAGE, big);
    assert_eq!(log.revision_of(PAGE, big), 1);
    assert_eq!(log.key[log.slot(0) as usize] as u64, (1u64 << 63) + 5);
}

#[test]
fn change_the_topic_count_is_bounded_by_the_u16_a_topic_travels_as() {
    assert!(std::panic::catch_unwind(|| log(0x10001, 4)).is_err());
    assert!(std::panic::catch_unwind(|| log(1, 0)).is_err());
}

#[test]
fn waker_a_pump_wakes_the_listeners_of_changed_keys_once_each_and_nothing_between_pumps() {
    let log = Rc::new(RefCell::new(log(2, 16)));
    let mut waker = ChangeWaker::new(log.clone());
    let (a, b, cell) = (Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)));
    let (a2, b2, cell2) = (a.clone(), b.clone(), cell.clone());
    waker.subscribe(PAGE, 1, Box::new(move || *a2.borrow_mut() += 1));
    let off_b = waker.subscribe(PAGE, 2, Box::new(move || *b2.borrow_mut() += 1));
    waker.subscribe(CELL, 1, Box::new(move || *cell2.borrow_mut() += 1));

    log.borrow_mut().bump(PAGE, 1);
    log.borrow_mut().bump(PAGE, 1);
    assert_eq!([*a.borrow(), *b.borrow(), *cell.borrow()], [0, 0, 0], "the log calls no one");
    assert_eq!(waker.pump(), 1);
    assert_eq!(
        [*a.borrow(), *b.borrow(), *cell.borrow()],
        [1, 0, 0],
        "twice in one span wakes once; the same key in another topic is another key"
    );

    waker.unsubscribe(off_b);
    log.borrow_mut().bump(PAGE, 2);
    assert_eq!(waker.pump(), 0);
    assert_eq!(waker.pump(), 0, "an empty span wakes nothing");
}

#[test]
fn waker_a_pump_whose_cursor_was_overwritten_wakes_every_listener_it_holds() {
    let log = Rc::new(RefCell::new(log(2, 2)));
    let mut waker = ChangeWaker::new(log.clone());
    let (a, cell) = (Rc::new(RefCell::new(0)), Rc::new(RefCell::new(0)));
    let (a2, cell2) = (a.clone(), cell.clone());
    waker.subscribe(PAGE, 1, Box::new(move || *a2.borrow_mut() += 1));
    waker.subscribe(CELL, 9, Box::new(move || *cell2.borrow_mut() += 1));
    for k in 10..15 {
        log.borrow_mut().bump(PAGE, k);
    }
    waker.pump();
    assert_eq!([*a.borrow(), *cell.borrow()], [1, 1]);
}

#[test]
fn waker_a_topic_listener_hears_each_changed_key_of_its_topic_once_per_pump_and_null_when_the_pump_was_lost() {
    let log = Rc::new(RefCell::new(log(2, 4)));
    let mut waker = ChangeWaker::new(log.clone());
    let heard: Rc<RefCell<Vec<Option<i64>>>> = Rc::new(RefCell::new(Vec::new()));
    let h = heard.clone();
    let off = waker.subscribe_topic(PAGE, Box::new(move |key| h.borrow_mut().push(key)));
    log.borrow_mut().bump(PAGE, 1);
    log.borrow_mut().bump(CELL, 1);
    log.borrow_mut().bump(PAGE, 2);
    log.borrow_mut().bump(PAGE, 1);
    waker.pump();
    assert_eq!(*heard.borrow(), [Some(1), Some(2)]);
    heard.borrow_mut().clear();
    for k in 0..6 {
        log.borrow_mut().bump(PAGE, k);
    }
    waker.pump();
    assert_eq!(*heard.borrow(), [None]);
    waker.unsubscribe(off);
    log.borrow_mut().bump(PAGE, 9);
    heard.borrow_mut().clear();
    waker.pump();
    assert!(heard.borrow().is_empty());
}
