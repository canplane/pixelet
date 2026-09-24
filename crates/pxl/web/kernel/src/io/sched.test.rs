// pxl-kernel/io/sched.test.rs
//
// The four things the three tables are for. Each is a property of how they
// are separated rather than of anything that has to be kept in step.

use super::*;
use crate::global::serial;
use crate::sched::ledger::record::{Failure, Outcome};
use xpute_runtime::abi::fetch::FetchFail;

/// `Global` hands out `&mut` from a shared static on the promise that one
/// thread runs the substrate, and every check here names the same one.
fn fresh() -> std::sync::MutexGuard<'static, ()> {
    let guard = serial();
    reset();
    set_credits(2);
    guard
}

fn rank(class: u8, distance: f64) -> Rank {
    Rank { class, distance }
}

/// Every key the pump handed out since the last look.
fn started() -> Vec<u64> {
    let mut out = Vec::new();
    take_started(|key, _, _| out.push(key));
    out
}

fn stopped() -> Vec<u64> {
    let mut out = Vec::new();
    take_stops(|key| out.push(key));
    out
}

fn ok(key: u64) {
    settle(key, Outcome::Ok, None, Failure::default(), None);
}

fn failed(key: u64) {
    settle(
        key,
        Outcome::Error,
        None,
        Failure { fail: FetchFail::Network, detail: 0 },
        crate::io::policy::retry_after(Outcome::Error),
    );
}

#[test]
fn saying_the_same_set_again_changes_nothing() {
    let _g = fresh();
    let want = [(1u64, rank(1, 0.0))];
    declare(Category::Terrain, &want);
    pump();
    let first = started();
    assert_eq!(first.len(), 1);
    for _ in 0..5 {
        declare(Category::Terrain, &want);
        pump();
        assert!(started().is_empty(), "already out, so nothing is started again");
    }
}

#[test]
fn an_attempt_ending_does_not_answer_the_demand() {
    let _g = fresh();
    declare(Category::Terrain, &[(1, rank(1, 0.0))]);
    pump();
    let key = started()[0];
    ok(key);
    // The producer has not spoken since, so the want is still standing — an
    // attempt is not an answer, and only a producer withdraws a demand.
    assert_eq!(pending_count(Category::Terrain), 1);
    pump();
    assert_eq!(started().len(), 1, "and it is tried again");
    // It goes when the producer stops saying it, and only then.
    declare(Category::Terrain, &[]);
    assert_eq!(pending_count(Category::Terrain), 1, "the attempt is still out");
    assert_eq!(counts().0, 0, "but nothing is waiting");
}

#[test]
fn a_failure_waits_rather_than_asking_straight_back() {
    let _g = fresh();
    declare(Category::Terrain, &[(1, rank(1, 0.0))]);
    pump();
    failed(started()[0]);
    // Still wanted — a failure is not a withdrawal — and not tried again yet.
    for _ in 0..5 {
        declare(Category::Terrain, &[(1, rank(1, 0.0))]);
        pump();
        assert!(started().is_empty(), "held back, not re-asked every turn");
    }
    assert_eq!(counts(), (1, 0), "wanted, nothing out");
}

#[test]
fn a_current_demand_is_said_whatever_old_work_is_draining() {
    let _g = fresh();
    let s = sched();
    // Both credits spent on work the producer has since stopped wanting.
    declare(Category::Terrain, &[(1, rank(2, 0.0)), (2, rank(2, 1.0))]);
    pump();
    let out = started();
    assert_eq!(out.len(), 2);
    declare(Category::Terrain, &[(3, rank(1, 0.0))]);
    assert_eq!(s.desired.len(), 1, "the new want is representable with both credits spent");
    pump();
    assert_eq!(stopped().len(), 1, "one obsolete attempt is told to stop");
    assert_eq!(s.attempt.iter().flatten().filter(|a| a.stopping).count(), 1, "and only one, since a stop is a credit on its way back");
    // Telling it to stop is not the end of it: it is still out and still
    // spending, and the credit comes back when the host says it is over.
    assert!(started().is_empty(), "nothing starts on a credit that has not come back");
    ok(out[0]);
    pump();
    assert_eq!(started().len(), 1, "and now it does");
}

#[test]
fn a_whole_class_better_takes_a_credit_and_a_nearer_one_does_not() {
    let _g = fresh();
    let s = sched();
    set_credits(1);
    declare(Category::Terrain, &[(1, rank(2, 5.0))]);
    pump();
    let _ = started();
    // Nearer, same class: not worth what the running attempt has spent.
    declare(Category::Terrain, &[(1, rank(2, 5.0)), (2, rank(2, 0.0))]);
    pump();
    assert!(stopped().is_empty(), "distance says which to start, not which to restart");
    // A class better: the waiting work is a different kind of wanted.
    declare(Category::Terrain, &[(1, rank(2, 5.0)), (3, rank(1, 9.0))]);
    pump();
    assert_eq!(stopped().len(), 1, "a hole takes the credit from a refinement");
    assert_eq!(s.desired.len(), 2, "and the one it took from is still wanted");
}

#[test]
fn an_asset_shares_the_credits_and_is_never_withdrawn() {
    let _g = fresh();
    set_credits(1);
    // A page the viewer is waiting on and an asset, both wanted at once.
    declare(Category::Terrain, &[(1, crate::io::policy::page(crate::world::pyramid::Resolved::Missing, false, 0.0))]);
    declare(Category::Asset, &[(7, crate::io::policy::asset())]);
    pump();
    assert_eq!(started().len(), 1, "one credit, one read — an asset does not go beside the pages");
    // The asset is still wanted while it waits: nothing about it is stale.
    assert_eq!(pending_count(Category::Asset), 1);
    assert_eq!(counts().0, 1, "and it is what is waiting");
}
