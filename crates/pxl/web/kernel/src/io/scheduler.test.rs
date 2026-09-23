// pxl-kernel/io/scheduler.test.rs
//
// How the queue orders, yields and counts is xpute_runtime::io::queue's to test.
// What is the world's here: the key a demand goes out and comes back under,
// the credits the central scheduler grants, and the ledger's record.

use xpute_runtime::abi::fetch::FetchFail;

use super::*;
use crate::global::serial;
use crate::sched::ledger::record::{ledger_recording_start, ledger_tasks, name_id, Failure, Outcome, TaskRow};
use crate::sched::ledger::spec::{CATEGORY_NAMES, OUTCOME_NAMES, SUMMARY, TASK_SUMMARY};
use crate::sched::ledger::summary::ledger_summarize;

fn d(category: Category, subject: u64, priority: f64) -> Demand {
    Demand {
        key: DemandKey::new(category, subject),
        priority,
    }
}

/// The turn's end: the pump, and the keys it started.
fn started() -> Vec<DemandKey> {
    fetch_pump();
    let mut keys = Vec::new();
    fetch_take_started(|d| keys.push(d.key));
    keys
}

fn aborts() -> Vec<DemandKey> {
    let mut keys = Vec::new();
    fetch_take_aborts(|k| keys.push(k));
    keys
}

fn fresh() -> std::sync::MutexGuard<'static, ()> {
    let g = serial();
    scheduler_reset();
    fetch_take_started(|_| {});
    fetch_take_aborts(|_| {});
    set_fetch_credits(6);
    g
}

#[test]
fn scheduler_a_started_demand_goes_out_under_the_key_it_was_asked_for() {
    let _g = fresh();
    let x = d(Category::Ocean, 0xcafe_5678_9abc_def0, 0.0);
    fetch_sync(Category::Ocean, std::slice::from_ref(&x));
    fetch_pump();
    let mut out = Vec::new();
    fetch_take_started(|d| out.push(*d));
    assert_eq!(out, vec![x]);
    fetch_settle(x.key, Outcome::Ok, None, Failure::default());
    assert_eq!(fetch_pending_count(Category::Ocean), 0);
}

#[test]
fn scheduler_starts_no_more_than_the_credits_the_central_scheduler_granted() {
    let _g = fresh();
    set_fetch_credits(2);
    let h: Vec<Demand> = (0..5).map(|i| d(Category::Poi, i, i as f64)).collect();
    fetch_sync(Category::Poi, &h);
    assert_eq!(started(), [h[0].key, h[1].key]);
    set_fetch_credits(0);
    fetch_settle(h[0].key, Outcome::Ok, None, Failure::default());
    assert!(started().is_empty());
}

#[test]
fn scheduler_an_aborted_demand_goes_out_under_its_key_and_settles_under_it() {
    let _g = fresh();
    // One credit, so the demand that replaces it has to take it.
    set_fetch_credits(1);
    let (keep, drop) = (d(Category::Terrain, 11, 0.0), d(Category::Terrain, 22, 1.0));
    fetch_sync(Category::Terrain, std::slice::from_ref(&drop));
    assert_eq!(started(), [drop.key]);
    fetch_sync(Category::Terrain, std::slice::from_ref(&keep));
    assert!(started().is_empty(), "the one credit is still out");
    assert_eq!(aborts(), vec![drop.key]);
    assert!(fetch_running(drop.key), "told to stop is not over");
    fetch_settle(drop.key, Outcome::Aborted, None, Failure::default());
    assert!(!fetch_running(drop.key));
    assert_eq!(fetch_pending_count(Category::Terrain), 1);
    assert_eq!(started(), [keep.key], "the credit it gave up");
}

#[test]
fn scheduler_ledger_records_outcome_for_completed_and_aborted_demands() {
    let _g = fresh();
    ledger_recording_start();
    let cat = Category::Poi;
    // One credit, so dropping `killed` from the wishlist is what takes it
    // back for `done` — an abort happens where a demand is short, and only
    // there.
    set_fetch_credits(1);
    let (done, killed) = (d(cat, 1, 1.0), d(cat, 2, 0.0));
    fetch_sync(cat, &[done, killed]);
    assert_eq!(started(), [killed.key]);
    fetch_sync(cat, std::slice::from_ref(&done)); // drops `killed`
    started();
    for key in aborts() {
        fetch_settle(key, Outcome::Aborted, None, Failure::default());
    }
    started();
    fetch_settle(done.key, Outcome::Ok, None, Failure::default());
    let c = name_id(CATEGORY_NAMES, cat.as_str());
    let mine: Vec<&TaskRow> = ledger_tasks().iter().filter(|r| r.category == c).collect();
    let outcome = |subject: u64| mine.iter().find(|r| r.subject == subject).map(|r| r.outcome);
    assert_eq!(outcome(done.key.subject), Some(name_id(OUTCOME_NAMES, Outcome::Ok.as_str())));
    assert_eq!(outcome(killed.key.subject), Some(name_id(OUTCOME_NAMES, Outcome::Aborted.as_str())));
    assert!(mine.iter().all(|r| !r.start_ms.is_nan()));
}

/// What the reason's two numbers have to keep apart, now that nothing
/// compares prose: one refused status from another, and a refusal from a
/// transport failure.
#[test]
fn a_failure_is_grouped_by_its_reason_and_its_status_not_by_any_text() {
    // `fresh`, not a bare `serial`: without the credits it grants, the pump
    // starts nothing, so nothing settles and the ledger has nothing to group.
    let _g = fresh();
    let cat = Category::Ocean;
    ledger_recording_start();
    let fails = [
        Failure { fail: FetchFail::Status, status: 503 },
        Failure { fail: FetchFail::Status, status: 503 },
        Failure { fail: FetchFail::Status, status: 429 },
        Failure { fail: FetchFail::Network, status: 0 },
    ];
    for (i, fail) in fails.into_iter().enumerate() {
        let demand = d(cat, i as u64, 0.0);
        fetch_sync(cat, &[demand]);
        started();
        fetch_settle(demand.key, Outcome::Error, None, fail);
    }
    let block = ledger_summarize(f64::INFINITY);
    let at = (SUMMARY::tasks::AT + SUMMARY::tasks::slots + name_id(CATEGORY_NAMES, cat.as_str()) as u32 * TASK_SUMMARY::WORDS) as usize;
    let reasons = at + TASK_SUMMARY::reasons as usize;
    assert_eq!(block[at + TASK_SUMMARY::reason_count as usize], 3.0, "503, 429 and network are three reasons");
    assert_eq!(&block[reasons..reasons + 3], &[FetchFail::Status as u32 as f64, 503.0, 2.0], "commonest first");
    assert_eq!(Failure { fail: FetchFail::Status, status: 503 }.to_string(), "status 503");
    assert_eq!(Failure { fail: FetchFail::Network, status: 0 }.to_string(), "network");
}

#[test]
fn scheduler_holds_a_whole_window_of_rings_beside_what_a_zoom_left_in_flight() {
    let _g = fresh();
    set_fetch_credits(u32::MAX);
    let rings = (WINDOW_LEVELS * HELD_AXIS * HELD_AXIS) as u64;
    // The window a zoom left: every request it could have running, then
    // aborted by the next window's wishlist, and still unsettled.
    let before: Vec<Demand> = (0..rings).map(|i| d(Category::Terrain, 1 << 40 | i, i as f64)).collect();
    assert_eq!(fetch_sync(Category::Terrain, &before), 0);
    assert_eq!(started().len(), MAX_CREDITS as usize);
    let after: Vec<Demand> = (0..rings).map(|i| d(Category::Terrain, 2 << 40 | i, i as f64)).collect();
    assert_eq!(fetch_sync(Category::Terrain, &after), 0, "a ring's page left unasked");
    assert!(started().is_empty(), "every credit is out on the window that left");
    assert_eq!(aborts().len(), MAX_CREDITS as usize, "and every one is taken back for the window that came");
    assert_eq!(fetch_sync(Category::Ocean, &[d(Category::Ocean, 0, 0.0)]), 0);
}

/// Invariant A at this layer: a wishlist is never turned away because
/// another category's is already held, and never because of anything on the
/// wire. Terrain's widest and the ocean tile are both asked for in one turn,
/// with every credit already spent on a window that has gone.
#[test]
fn scheduler_every_category_can_state_its_whole_wishlist_at_once() {
    let _g = fresh();
    set_fetch_credits(MAX_CREDITS);
    let rings = TERRAIN_WANTED as u64;
    let gone: Vec<Demand> = (0..rings).map(|i| d(Category::Terrain, 3 << 40 | i, i as f64)).collect();
    fetch_sync(Category::Terrain, &gone);
    assert_eq!(started().len(), MAX_CREDITS as usize);
    let now: Vec<Demand> = (0..rings).map(|i| d(Category::Terrain, 4 << 40 | i, i as f64)).collect();
    assert_eq!(fetch_sync(Category::Terrain, &now), 0, "terrain's whole wishlist");
    assert_eq!(fetch_sync(Category::Ocean, &[d(Category::Ocean, 0, 0.0)]), 0, "and the ocean tile beside it");
}
