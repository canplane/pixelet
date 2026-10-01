// pxl-kernel/io/sched.test.rs
//
// What the ledger keeps whatever its serve policy: each is a property of how
// the two tables are separated rather than of anything kept in step. The
// policy here orders by why and then by distance, preempts for a better why,
// and waits two seconds after a failure; the kernel's own order is
// policy.test.rs's.

use super::*;
use crate::io::demand::Why;
use crate::sched::profiler::record::{Failure, Outcome};
use crate::world::observation::Order;
use xpute_runtime::abi::fetch::FetchFail;

struct ByWhy;

impl Serve for ByWhy {
    fn rank(&self, _category: Category, _subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        Rank {
            class: ask.why as u8,
            distance: ask.order.near,
        }
    }

    fn preempts(&self, waiting: Rank, running: Rank) -> bool {
        waiting.class < running.class
    }

    fn intake(&self) -> u32 {
        u32::MAX
    }

    fn retry_after(&self, outcome: Outcome) -> Option<f64> {
        (outcome == Outcome::error).then_some(2_000.0)
    }
}

fn ledger() -> Ledger<ByWhy> {
    let mut l = Ledger::new(ByWhy);
    l.set_credits(2);
    l
}

fn ask(why: Why, near: f64) -> Ask {
    Ask {
        why,
        order: Order { seen: true, near },
    }
}

/// Every key the pump handed out since the last look.
fn started<S: Serve>(l: &mut Ledger<S>) -> Vec<u64> {
    let mut out = Vec::new();
    l.take_started(|key, _, _| out.push(key));
    out
}

fn stopped<S: Serve>(l: &mut Ledger<S>) -> Vec<u64> {
    let mut out = Vec::new();
    l.take_stops(|key| out.push(key));
    out
}

fn ok<S: Serve>(l: &mut Ledger<S>, key: u64, at: f64) {
    l.settle(key, Outcome::ok, None, Failure::default(), at);
}

fn failed<S: Serve>(l: &mut Ledger<S>, key: u64, at: f64) {
    let fail = Failure {
        fail: FetchFail::Network,
        ..Failure::default()
    };
    l.settle(key, Outcome::error, None, fail, at);
}

#[test]
fn saying_the_same_set_again_changes_nothing() {
    let mut l = ledger();
    let want = [(1u64, ask(Why::Fault, 0.0))];
    l.declare(Category::terrain, &want, 0.0);
    l.pump(0.0);
    assert_eq!(started(&mut l).len(), 1);
    for _ in 0..5 {
        l.declare(Category::terrain, &want, 0.0);
        l.pump(0.0);
        assert!(started(&mut l).is_empty(), "already out, so nothing is started again");
    }
}

#[test]
fn an_attempt_ending_does_not_answer_the_demand() {
    let mut l = ledger();
    l.declare(Category::terrain, &[(1, ask(Why::Fault, 0.0))], 0.0);
    l.pump(0.0);
    let key = started(&mut l)[0];
    ok(&mut l, key, 0.0);
    // The producer has not spoken since, so the want is still standing — an
    // attempt is not an answer, and only a producer withdraws a demand.
    assert_eq!(l.pending_count(Category::terrain), 1);
    l.pump(0.0);
    assert_eq!(started(&mut l).len(), 1, "and it is tried again");
    // It goes when the producer stops saying it, and only then.
    l.declare(Category::terrain, &[], 0.0);
    assert_eq!(l.pending_count(Category::terrain), 1, "the attempt is still out");
    assert_eq!(l.counts().0, 0, "but nothing is waiting");
}

#[test]
fn a_failure_waits_rather_than_asking_straight_back() {
    let mut l = ledger();
    l.declare(Category::terrain, &[(1, ask(Why::Fault, 0.0))], 0.0);
    l.pump(0.0);
    let key = started(&mut l)[0];
    failed(&mut l, key, 0.0);
    // Still wanted — a failure is not a withdrawal — and not tried again yet.
    for t in 0..5 {
        l.declare(Category::terrain, &[(1, ask(Why::Fault, 0.0))], t as f64);
        l.pump(t as f64);
        assert!(started(&mut l).is_empty(), "held back, not re-asked every turn");
    }
    assert_eq!(l.counts(), (1, 0), "wanted, nothing out");
    l.pump(2_000.0);
    assert_eq!(started(&mut l).len(), 1, "and tried again once the wait is over");
}

#[test]
fn a_current_demand_is_said_whatever_old_work_is_draining() {
    let mut l = ledger();
    // Both credits spent on work the producer has since stopped wanting.
    l.declare(Category::terrain, &[(1, ask(Why::Hint, 0.0)), (2, ask(Why::Hint, 1.0))], 0.0);
    l.pump(0.0);
    let out = started(&mut l);
    assert_eq!(out.len(), 2);
    l.declare(Category::terrain, &[(3, ask(Why::Fault, 0.0))], 0.0);
    assert_eq!(l.counts().0, 1, "the new want is representable with both credits spent");
    l.pump(0.0);
    // What the camera left is stopped the turn it leaves, all of it, and its
    // credits start what is wanted now in the same turn.
    assert_eq!(stopped(&mut l).len(), 2, "every obsolete attempt is told to stop");
    assert_eq!(started(&mut l).len(), 1, "and the current want starts at once");
    // What the host still reports under a stopped key names nothing.
    ok(&mut l, out[0], 0.0);
    assert_eq!(l.counts(), (0, 1), "the late report neither frees nor ends anything");
}

#[test]
fn a_running_attempt_is_stopped_only_for_one_the_policy_ranks_ahead_and_asks_it() {
    let mut l = ledger();
    l.set_credits(1);
    l.declare(Category::terrain, &[(1, ask(Why::Hint, 5.0))], 0.0);
    l.pump(0.0);
    let _ = started(&mut l);
    // Nearer, same class: the policy does not ask it.
    l.declare(Category::terrain, &[(1, ask(Why::Hint, 5.0)), (2, ask(Why::Hint, 0.0))], 0.0);
    l.pump(0.0);
    assert!(stopped(&mut l).is_empty(), "distance says which to start, not which to restart");
    // A class better: the policy asks it.
    l.declare(Category::terrain, &[(1, ask(Why::Hint, 5.0)), (3, ask(Why::Fault, 9.0))], 0.0);
    l.pump(0.0);
    assert_eq!(stopped(&mut l).len(), 1, "a class better takes the credit");
    assert_eq!(started(&mut l).len(), 1, "in the same turn");
    assert_eq!(l.counts().0, 1, "and the one it took from is still wanted");
}

/// A policy that would stop anything for anything: the ledger lets it stop
/// only for a want strictly ahead in its total order, so at rest the stops
/// end and every want is served.
struct Restless;

impl Serve for Restless {
    fn rank(&self, _category: Category, _subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        Rank { class: 0, distance: ask.order.near }
    }

    fn preempts(&self, _waiting: Rank, _running: Rank) -> bool {
        true
    }

    fn intake(&self) -> u32 {
        1
    }

    fn retry_after(&self, _outcome: Outcome) -> Option<f64> {
        None
    }
}

#[test]
fn a_policy_that_always_preempts_still_serves_every_want_at_rest() {
    let mut l = Ledger::new(Restless);
    l.set_credits(2);
    let mut wanted: Vec<(u64, Ask)> = (0..12).map(|k| (k, ask(Why::Fault, (k * 7 % 12) as f64))).collect();
    let mut out: Vec<u64> = Vec::new();
    for turn in 0..200 {
        l.declare(Category::poi, &wanted, turn as f64);
        l.pump(turn as f64);
        let stops = stopped(&mut l);
        out.retain(|k| !stops.contains(k));
        out.extend(started(&mut l));
        assert!(out.len() <= 2, "never more out than the credits");
        // The host answers the oldest attempt each turn, and the producer
        // stops wanting what was answered.
        if let Some(key) = out.first().copied() {
            let (_, subject) = l.attempt_of(key).expect("out");
            ok(&mut l, key, turn as f64);
            out.remove(0);
            wanted.retain(|&(s, _)| s != subject);
        }
        if wanted.is_empty() {
            return;
        }
    }
    panic!("{} wants never served", wanted.len());
}

#[test]
fn an_asset_shares_the_credits_and_is_never_withdrawn() {
    let mut l = ledger();
    l.set_credits(1);
    // A page the viewer is waiting on and an asset, both wanted at once.
    l.declare(Category::terrain, &[(1, ask(Why::Fault, 0.0))], 0.0);
    l.declare(Category::asset, &[(7, Ask::PLAIN)], 0.0);
    l.pump(0.0);
    assert_eq!(started(&mut l).len(), 1, "one credit, one read — an asset does not go beside the pages");
    // The asset is still wanted while it waits: nothing about it is stale.
    assert_eq!(l.pending_count(Category::asset), 1);
    assert_eq!(l.counts().0, 1, "and it is what is waiting");
}
