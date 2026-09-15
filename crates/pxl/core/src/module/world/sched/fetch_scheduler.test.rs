// pxl_core/module/world/sched/fetch_scheduler.test.rs
//
// How the queue orders, yields and counts is xpute_core::sched::io's to test.
// What is the world's here: the name and category a demand goes out and
// comes back under, the credits the central scheduler grants, and the
// ledger's record.

use super::*;
use crate::global::serial;
use crate::module::world::sched::ledger::{ledger_recording_start, ledger_snapshot, Outcome};

fn d(key: &str, category: &'static str, priority: f64) -> Demand {
    Demand {
        key: DemandKey::of(key),
        category,
        priority,
    }
}

/// The turn's end: the pump, and the keys it started.
fn started() -> Vec<String> {
    fetch_pump();
    let mut keys = Vec::new();
    fetch_take_started(|d| keys.push(d.key.to_string()));
    keys
}

fn aborts() -> Vec<String> {
    let mut keys = Vec::new();
    fetch_take_aborts(|k| keys.push(k.to_string()));
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
fn fetch_scheduler_a_started_demand_goes_out_under_the_name_and_category_it_was_asked_for() {
    let _g = fresh();
    let x = d("search:café 서울", "search", 0.0);
    fetch_sync("search", std::slice::from_ref(&x));
    fetch_pump();
    let mut out = Vec::new();
    fetch_take_started(|d| out.push(*d));
    assert_eq!(out, vec![x]);
    fetch_settle(&x.key, Outcome::Ok, None, None);
    assert_eq!(fetch_pending_count("search"), 0);
}

#[test]
fn fetch_scheduler_starts_no_more_than_the_credits_the_central_scheduler_granted() {
    let _g = fresh();
    set_fetch_credits(2);
    let h: Vec<Demand> = (0..5).map(|i| d(&format!("credit:{i}"), "credit", i as f64)).collect();
    fetch_sync("credit", &h);
    assert_eq!(started(), ["credit:0", "credit:1"]);
    set_fetch_credits(0);
    fetch_settle("credit:0", Outcome::Ok, None, None);
    assert!(started().is_empty());
}

#[test]
fn fetch_scheduler_an_aborted_demand_goes_out_under_its_name_and_settles_under_it() {
    let _g = fresh();
    let (keep, drop) = (d("yield:keep", "yield", 0.0), d("yield:drop", "yield", 1.0));
    fetch_sync("yield", &[keep, drop]);
    assert_eq!(started().len(), 2);
    fetch_sync("yield", std::slice::from_ref(&keep));
    assert_eq!(aborts(), vec![drop.key.to_string()]);
    assert!(fetch_running(&drop.key));
    fetch_settle(&drop.key, Outcome::Aborted, None, None);
    assert!(!fetch_running(&drop.key));
    assert_eq!(fetch_pending_count("yield"), 1);
}

#[test]
fn fetch_scheduler_ledger_records_outcome_for_completed_and_aborted_demands() {
    let _g = fresh();
    ledger_recording_start();
    let cat = "ledger";
    let (done, killed) = (d("ledger:ok", cat, 0.0), d("ledger:killed", cat, 1.0));
    fetch_sync(cat, &[done, killed]);
    started();
    fetch_settle(&done.key, Outcome::Ok, None, None);
    fetch_sync(cat, std::slice::from_ref(&done)); // drops `killed`
    for key in aborts() {
        fetch_settle(&key, Outcome::Aborted, None, None);
    }
    let mine: Vec<_> = ledger_snapshot().iter().filter(|r| r.category == cat).collect();
    assert_eq!(mine.iter().find(|r| r.key == done.key).map(|r| r.outcome), Some(Outcome::Ok));
    assert_eq!(mine.iter().find(|r| r.key == killed.key).map(|r| r.outcome), Some(Outcome::Aborted));
    assert!(mine.iter().all(|r| r.start_ms.is_some()));
}

#[test]
fn error_reason_an_http_failure_is_grouped_by_its_status_anything_else_by_its_kind() {
    assert_eq!(error_reason("Error", "[geocode/5788:1842:0] http 503").as_str(), "http 503");
    assert_eq!(error_reason("Error", "[tile/1:2:0] http 429").as_str(), "http 429");
    assert_eq!(error_reason("TypeError", "Failed to fetch").as_str(), "TypeError: Failed to fetch");
    assert_eq!(error_reason("string", "boom").as_str(), "string: boom");
}
