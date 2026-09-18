// pxl-kernel/io/fetch_scheduler.test.rs
//
// How the queue orders, yields and counts is xpute_runtime::sched::io's to test.
// What is the world's here: the key a demand goes out and comes back under,
// the credits the central scheduler grants, and the ledger's record.

use pxl_spec::fetch::FetchFail;

use super::*;
use crate::sched::ledger::record::{ledger_recording_start, ledger_tasks, name_id, Failure, Outcome, TaskRow};
use crate::sched::ledger::spec::{CATEGORY_NAMES, OUTCOME_NAMES, SUMMARY, TASK_SUMMARY};
use crate::sched::ledger::summary::ledger_summarize;
use crate::sys::global::serial;

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
    fetch_scheduler_reset();
    fetch_take_started(|_| {});
    fetch_take_aborts(|_| {});
    set_fetch_credits(6);
    g
}

#[test]
fn fetch_scheduler_a_started_demand_goes_out_under_the_key_it_was_asked_for() {
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
fn fetch_scheduler_starts_no_more_than_the_credits_the_central_scheduler_granted() {
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
fn fetch_scheduler_an_aborted_demand_goes_out_under_its_key_and_settles_under_it() {
    let _g = fresh();
    let (keep, drop) = (d(Category::Terrain, 11, 0.0), d(Category::Terrain, 22, 1.0));
    fetch_sync(Category::Terrain, &[keep, drop]);
    assert_eq!(started().len(), 2);
    fetch_sync(Category::Terrain, std::slice::from_ref(&keep));
    assert_eq!(aborts(), vec![drop.key]);
    assert!(fetch_running(drop.key));
    fetch_settle(drop.key, Outcome::Aborted, None, Failure::default());
    assert!(!fetch_running(drop.key));
    assert_eq!(fetch_pending_count(Category::Terrain), 1);
}

#[test]
fn fetch_scheduler_ledger_records_outcome_for_completed_and_aborted_demands() {
    let _g = fresh();
    ledger_recording_start();
    let cat = Category::Poi;
    let (done, killed) = (d(cat, 1, 0.0), d(cat, 2, 1.0));
    fetch_sync(cat, &[done, killed]);
    started();
    fetch_settle(done.key, Outcome::Ok, None, Failure::default());
    fetch_sync(cat, std::slice::from_ref(&done)); // drops `killed`
    for key in aborts() {
        fetch_settle(key, Outcome::Aborted, None, Failure::default());
    }
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
