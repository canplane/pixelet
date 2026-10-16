// pxl-kernel/io/request.test.rs
//
// What the request scheduler keeps whatever its policy. The policy here orders by why
// then distance, blames by errno as the kernel's does, and waits 2 s after a failure,
// giving the server up after its second.

use super::*;
use crate::io::demand::Why;
use xpute_kit::status::errno::Errno;

use crate::io::demand::Order;
use crate::io::failure::{Ended, Failure};

struct ByWhy;

impl Serve for ByWhy {
    fn rank(&self, _category: Stream, _subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        Rank {
            class: ask.why as u8,
            distance: ask.order.near,
            detail: 0,
        }
    }

    fn intake(&self) -> u32 {
        u32::MAX
    }

    fn blame(&self, failure: Failure) -> Blame {
        match failure.errno {
            Errno::ENETUNREACH => Blame::Network,
            Errno::EACCES => Blame::Server,
            _ => Blame::Object,
        }
    }

    fn backoff(&self, strikes: u32, _seed: u64) -> Option<f64> {
        (strikes <= 2).then_some(2_000.0)
    }
}

/// Room for every test's wants in each stream.
const BOUNDS: Bounds = [64; Stream::ALL.len()];

fn scheduler() -> RequestScheduler<ByWhy> {
    let mut l = RequestScheduler::new(ByWhy, BOUNDS);
    l.set_credits(2);
    l
}

fn ask(why: Why, near: f64) -> Ask {
    Ask {
        why,
        order: Order { seen: true, near },
    }
}

/// A class worse than any seen ask.
fn unseen(near: f64) -> Ask {
    Ask {
        why: Why::Demand,
        order: Order { seen: false, near },
    }
}

fn started<S: Serve>(l: &mut RequestScheduler<S>) -> Vec<u64> {
    let mut out = Vec::new();
    l.take_started(|credit, _, _| out.push(credit.key()));
    out
}

fn stopped<S: Serve>(l: &mut RequestScheduler<S>) -> Vec<u64> {
    let mut out = Vec::new();
    l.take_stops(|key| out.push(key));
    out
}

fn ok<S: Serve>(l: &mut RequestScheduler<S>, key: u64, at: f64) {
    l.settle(key, Ended::Ok, None, at);
}

fn failed<S: Serve>(l: &mut RequestScheduler<S>, key: u64, at: f64) {
    failed_with(l, key, Errno::ENETUNREACH, at);
}

fn failed_with<S: Serve>(l: &mut RequestScheduler<S>, key: u64, errno: Errno, at: f64) {
    let fail = Failure { errno, detail: 0, at: None };
    l.settle(key, Ended::Failed(fail), None, at);
}

#[test]
fn saying_the_same_set_again_changes_nothing() {
    let mut l = scheduler();
    let want = [(1u64, ask(Why::Demand, 0.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    assert_eq!(started(&mut l).len(), 1);
    for _ in 0..5 {
        l.declare(Stream::terrain, &want, 0.0);
        l.pump(0.0);
        assert!(started(&mut l).is_empty(), "already out, so nothing is started again");
    }
}

#[test]
fn an_answer_is_asked_once() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0))], 0.0);
    l.pump(0.0);
    let key = started(&mut l)[0];
    ok(&mut l, key, 0.0);
    l.pump(0.0);
    assert!(started(&mut l).is_empty(), "an answered want was asked again");
    assert!(stopped(&mut l).is_empty());
    assert_eq!(l.stream_counts(Stream::terrain), (0, 0));
    l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0))], 1.0);
    l.pump(1.0);
    assert_eq!(started(&mut l).len(), 1);
}

#[test]
fn a_failure_waits_rather_than_asking_straight_back() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0))], 0.0);
    l.pump(0.0);
    let key = started(&mut l)[0];
    failed(&mut l, key, 0.0);
    // A failure is not a withdrawal.
    for t in 0..5 {
        l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0))], t as f64);
        l.pump(t as f64);
        assert!(started(&mut l).is_empty(), "held back, not re-asked every turn");
    }
    assert_eq!(l.counts(), (1, 0), "wanted, nothing out");
    l.pump(2_000.0);
    assert_eq!(started(&mut l).len(), 1, "and tried again once the wait is over");
}

#[test]
fn an_object_that_fails_is_dropped_and_holds_nothing_else_back() {
    let mut l = scheduler();
    let want = [(1u64, ask(Why::Demand, 0.0)), (2, ask(Why::Demand, 1.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let out = started(&mut l);
    failed_with(&mut l, out[0], Errno::ENXIO, 0.0);
    ok(&mut l, out[1], 0.0);
    for t in 0..5 {
        l.declare(Stream::terrain, &want[..1], t as f64 * 1_000.0);
        l.pump(t as f64 * 1_000.0);
        assert!(started(&mut l).is_empty(), "a missing object is not asked again");
    }
    l.declare(Stream::terrain, &[(3, ask(Why::Demand, 0.0))], 5_000.0);
    l.pump(5_000.0);
    assert_eq!(started(&mut l).len(), 1, "and the server is not held for it");
}

#[test]
fn a_failing_server_holds_every_start_then_probes_one_then_is_given_up() {
    let mut l = scheduler();
    let want: Vec<(u64, Ask)> = (0..4).map(|k| (k, ask(Why::Demand, k as f64))).collect();
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let out = started(&mut l);
    failed_with(&mut l, out[0], Errno::EACCES, 0.0);
    l.pump(1_000.0);
    assert!(started(&mut l).is_empty(), "held: the credit freed starts nothing");
    l.pump(2_000.0);
    assert_eq!(started(&mut l).len(), 0, "one out already: the other attempt is the probe");
    failed_with(&mut l, out[1], Errno::EACCES, 2_000.0);
    assert_eq!(l.strikes, 1, "out before the strike, so the same outage");
    let mut at = 2_000.0;
    for strike in 2..=3 {
        l.pump(at);
        let probe = started(&mut l);
        assert_eq!(probe.len(), 1, "one probe after the wait");
        failed_with(&mut l, probe[0], Errno::EACCES, at);
        assert_eq!(l.strikes, strike);
        at += 2_000.0;
    }
    assert!(l.given_up, "past the last wait");
    l.pump(100_000.0);
    assert!(started(&mut l).is_empty(), "nothing is asked once given up");
}

#[test]
fn what_was_out_when_the_server_failed_counts_as_one_failure() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0)), (2, ask(Why::Demand, 1.0))], 0.0);
    l.pump(0.0);
    for key in started(&mut l) {
        failed_with(&mut l, key, Errno::EACCES, 10.0);
    }
    assert_eq!(l.strikes, 1, "one outage, one wait");
    assert!(!l.given_up);
}

#[test]
fn a_current_demand_is_said_whatever_old_work_is_draining() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, unseen(0.0)), (2, unseen(1.0))], 0.0);
    l.pump(0.0);
    let out = started(&mut l);
    assert_eq!(out.len(), 2);
    l.declare(Stream::terrain, &[(3, ask(Why::Demand, 0.0))], 0.0);
    assert_eq!(l.counts().0, 1, "the new want is representable with both credits spent");
    l.pump(0.0);
    // What the camera left is stopped the turn it leaves, and its credits
    // start what is wanted now in the same turn.
    assert_eq!(stopped(&mut l).len(), 2, "every obsolete attempt is told to stop");
    assert_eq!(started(&mut l).len(), 1, "and the current want starts at once");
    ok(&mut l, out[0], 0.0);
    assert_eq!(l.counts(), (0, 1), "the late report neither frees nor ends anything");
}

#[test]
fn a_running_attempt_is_never_stopped_for_a_better_want() {
    let mut l = scheduler();
    l.set_credits(1);
    l.declare(Stream::terrain, &[(1, unseen(5.0))], 0.0);
    l.pump(0.0);
    let _ = started(&mut l);
    l.declare(Stream::terrain, &[(1, unseen(5.0)), (2, unseen(0.0))], 0.0);
    l.pump(0.0);
    assert!(stopped(&mut l).is_empty(), "distance says which to start, not which to restart");
    l.declare(Stream::terrain, &[(1, unseen(5.0)), (3, ask(Why::Demand, 9.0))], 0.0);
    l.pump(0.0);
    assert!(stopped(&mut l).is_empty(), "a class better waits for a free credit");
    assert!(started(&mut l).is_empty(), "and starts nothing meanwhile");
}

/// Takes one want a pump, every want ranked alike.
struct Restless;

impl Serve for Restless {
    fn rank(&self, _category: Stream, _subject: u64, ask: Ask, _enqueue_ms: f64) -> Rank {
        Rank {
            class: 0,
            distance: ask.order.near,
            detail: 0,
        }
    }

    fn intake(&self) -> u32 {
        1
    }

    fn blame(&self, _failure: Failure) -> Blame {
        Blame::Network
    }

    fn backoff(&self, _strikes: u32, _seed: u64) -> Option<f64> {
        Some(0.0)
    }
}

#[test]
fn every_want_is_served_at_rest() {
    let mut l = RequestScheduler::new(Restless, BOUNDS);
    l.set_credits(2);
    let mut wanted: Vec<(u64, Ask)> = (0..12).map(|k| (k, ask(Why::Demand, (k * 7 % 12) as f64))).collect();
    let mut out: Vec<u64> = Vec::new();
    for turn in 0..200 {
        l.declare(Stream::place, &wanted, turn as f64);
        l.pump(turn as f64);
        let stops = stopped(&mut l);
        out.retain(|k| !stops.contains(k));
        out.extend(started(&mut l));
        assert!(out.len() <= 2, "never more out than the credits");
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
    let mut l = scheduler();
    l.set_credits(1);
    l.declare(Stream::terrain, &[(1, ask(Why::Demand, 0.0))], 0.0);
    l.declare(Stream::asset, &[(7, Ask::PLAIN)], 0.0);
    l.pump(0.0);
    assert_eq!(started(&mut l).len(), 1, "one credit, one read — an asset does not go beside the tiles");
    let (waiting, out) = l.stream_counts(Stream::asset);
    assert_eq!(waiting + out, 1);
    assert_eq!(l.counts().0, 1, "and it is what is waiting");
}
