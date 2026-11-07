// pxl-kernel/sys/io/request.test.rs
//
// What the request scheduler keeps whatever its policy. The policy here orders the seen
// before the unseen, then by distance, blames by errno as the kernel's does, and waits 2 s after a failure,
// giving the server up after its second.

use super::*;
use xpute_kit::status::errno::Errno;

use crate::sys;
use crate::sys::io::demand::Order;
use crate::sys::io::failure::{Ended, Failure};

struct BySeen;

impl Serve for BySeen {
    fn rank(&self, _category: Stream, _subject: u64, order: Order, _enqueue_ms: f64) -> Rank {
        Rank {
            class: !order.seen as u8,
            dist: order.near,
            detail: 0,
        }
    }

    fn intake(&self) -> u32 {
        u32::MAX
    }

    fn fault(&self, failure: Failure) -> Fault {
        match failure.errnum {
            Errno::ENETUNREACH => Fault::Network,
            Errno::EACCES => Fault::Server,
            _ => Fault::Object,
        }
    }

    fn backoff(&self, failures: u32, _seed: u64) -> Option<f64> {
        (failures <= 2).then_some(2_000.0)
    }
}

/// Room for every test's wants in each stream.
const BOUNDS: Bounds = [64; Stream::ALL.len()];

fn scheduler() -> RequestScheduler<BySeen> {
    let mut l = RequestScheduler::new(BySeen, BOUNDS);
    l.set_credits(2);
    l
}

fn seen(near: f64) -> Order {
    Order { seen: true, near }
}

/// A class worse than any seen want.
fn unseen(near: f64) -> Order {
    Order { seen: false, near }
}

/// The keys started and stopped since last taken, every event taken.
fn taken<S: Serve>(l: &mut RequestScheduler<S>) -> (Vec<u64>, Vec<u64>) {
    let (mut starts, mut stops) = (Vec::new(), Vec::new());
    while let Some(event) = l.take_event() {
        match event {
            Event::Start(credit, _, _) => starts.push(credit.key()),
            Event::Stop(key) => stops.push(key),
            Event::End(..) => {}
        }
    }
    (starts, stops)
}

fn ok<S: Serve>(l: &mut RequestScheduler<S>, key: u64, at: f64) {
    l.settle(key, Ended::Ok, None, at);
}

fn failed<S: Serve>(l: &mut RequestScheduler<S>, key: u64, at: f64) {
    failed_with(l, key, Errno::ENETUNREACH, at);
}

fn failed_with<S: Serve>(l: &mut RequestScheduler<S>, key: u64, errnum: Errno, at: f64) {
    let fail = Failure {
        errnum,
        detail: 0,
        at: None,
    };
    l.settle(key, Ended::Failed(fail), None, at);
}

#[test]
fn same_set_no_change() {
    let mut l = scheduler();
    let want = [(1u64, seen(0.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    assert_eq!(taken(&mut l).0.len(), 1);
    for _ in 0..5 {
        l.declare(Stream::terrain, &want, 0.0);
        l.pump(0.0);
        assert!(
            taken(&mut l).0.is_empty(),
            "already out, so nothing is started again"
        );
    }
}

#[test]
fn answer_asked_once() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, seen(0.0))], 0.0);
    l.pump(0.0);
    let key = taken(&mut l).0[0];
    ok(&mut l, key, 0.0);
    l.pump(0.0);
    let (starts, stops) = taken(&mut l);
    assert!(starts.is_empty(), "an answered want was asked again");
    assert!(stops.is_empty());
    assert_eq!(l.stream_counts(Stream::terrain), (0, 0));
    l.declare(Stream::terrain, &[(1, seen(0.0))], 1.0);
    l.pump(1.0);
    assert_eq!(taken(&mut l).0.len(), 1);
}

#[test]
fn failure_backs_off() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, seen(0.0))], 0.0);
    l.pump(0.0);
    let key = taken(&mut l).0[0];
    failed(&mut l, key, 0.0);
    // A failure is not a withdrawal.
    for t in 0..5 {
        l.declare(Stream::terrain, &[(1, seen(0.0))], t as f64);
        l.pump(t as f64);
        assert!(
            taken(&mut l).0.is_empty(),
            "held back, not re-asked every turn"
        );
    }
    assert_eq!(
        l.stream_counts(Stream::terrain),
        (1, 0),
        "wanted, nothing out"
    );
    l.pump(2_000.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "and tried again once the wait is over"
    );
}

#[test]
fn failed_object_dropped() {
    let mut l = scheduler();
    let want = [(1u64, seen(0.0)), (2, seen(1.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let out = taken(&mut l).0;
    failed_with(&mut l, out[0], Errno::ENXIO, 0.0);
    ok(&mut l, out[1], 0.0);
    for t in 0..5 {
        l.declare(Stream::terrain, &want[..1], t as f64 * 1_000.0);
        l.pump(t as f64 * 1_000.0);
        assert!(
            taken(&mut l).0.is_empty(),
            "a missing object is not asked again"
        );
    }
    l.declare(Stream::terrain, &[(3, seen(0.0))], 5_000.0);
    l.pump(5_000.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "and the server is not held for it"
    );
}

#[test]
fn heap_refusal_waits_release() {
    use crate::sys::boot::set_heap_stat;
    use core::sync::atomic::{AtomicU32, Ordering};
    use xpute_core::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    let mut l = scheduler();
    let want = [(1u64, seen(0.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let key = taken(&mut l).0[0];
    l.settle(key, Ended::Refused(Refusal::now()), None, 0.0);
    for t in 0..5 {
        l.declare(Stream::terrain, &want, t as f64 * 10_000.0);
        l.pump(t as f64 * 10_000.0);
        assert!(taken(&mut l).0.is_empty(), "no time ends a heap's refusal");
    }
    let both = [want[0], (2, seen(0.0))];
    l.declare(Stream::terrain, &both, 50_000.0);
    l.pump(50_000.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "and the server is not held for it"
    );
    RELEASES.fetch_add(1, Ordering::Relaxed);
    l.declare(Stream::terrain, &both, 50_000.0);
    l.pump(50_000.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "asked again once the heap gives some back"
    );
    set_heap_stat(HeapStat::default);
}

#[test]
fn early_release_not_rewaited() {
    use crate::sys::boot::set_heap_stat;
    use core::sync::atomic::{AtomicU32, Ordering};
    use xpute_core::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    let mut l = scheduler();
    let want = [(1u64, seen(0.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let key = taken(&mut l).0[0];
    // Refused, then room given back before the refusal is settled.
    let refused = Refusal::now();
    RELEASES.fetch_add(1, Ordering::Relaxed);
    l.settle(key, Ended::Refused(refused), None, 0.0);
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "the release after the refusal already ends its wait"
    );
    set_heap_stat(HeapStat::default);
}

#[test]
fn failing_server_probe() {
    let mut l = scheduler();
    let want: Vec<(u64, Order)> = (0..4).map(|k| (k, seen(k as f64))).collect();
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let out = taken(&mut l).0;
    failed_with(&mut l, out[0], Errno::EACCES, 0.0);
    l.pump(1_000.0);
    assert!(
        taken(&mut l).0.is_empty(),
        "held: the credit freed starts nothing"
    );
    l.pump(2_000.0);
    assert_eq!(
        taken(&mut l).0.len(),
        0,
        "one out already: the other attempt is the probe"
    );
    failed_with(&mut l, out[1], Errno::EACCES, 2_000.0);
    assert_eq!(l.failures, 1, "out before the failure, so the same outage");
    let mut at = 2_000.0;
    for n in 2..=3 {
        l.pump(at);
        let probe = taken(&mut l).0;
        assert_eq!(probe.len(), 1, "one probe after the wait");
        failed_with(&mut l, probe[0], Errno::EACCES, at);
        assert_eq!(l.failures, n);
        at += 2_000.0;
    }
    assert!(l.given_up, "past the last wait");
    l.pump(100_000.0);
    assert!(taken(&mut l).0.is_empty(), "nothing is asked once given up");
}

#[test]
fn outage_counts_once() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, seen(0.0)), (2, seen(1.0))], 0.0);
    l.pump(0.0);
    for key in taken(&mut l).0 {
        failed_with(&mut l, key, Errno::EACCES, 10.0);
    }
    assert_eq!(l.failures, 1, "one outage, one wait");
    assert!(!l.given_up);
}

#[test]
fn cur_demand_said() {
    let mut l = scheduler();
    l.declare(Stream::terrain, &[(1, unseen(0.0)), (2, unseen(1.0))], 0.0);
    l.pump(0.0);
    let out = taken(&mut l).0;
    assert_eq!(out.len(), 2);
    l.declare(Stream::terrain, &[(3, seen(0.0))], 0.0);
    assert_eq!(
        l.stream_counts(Stream::terrain).0,
        1,
        "the new want is representable with both credits spent"
    );
    l.pump(0.0);
    // What the camera left is stopped the turn it leaves, and its credits
    // start what is wanted now in the same turn.
    let (starts, stops) = taken(&mut l);
    assert_eq!(stops.len(), 2, "every obsolete attempt is told to stop");
    assert_eq!(starts.len(), 1, "and the current want starts at once");
    ok(&mut l, out[0], 0.0);
    assert_eq!(
        l.stream_counts(Stream::terrain),
        (0, 1),
        "the late report neither frees nor ends anything"
    );
}

#[test]
fn running_attempt_kept() {
    let mut l = scheduler();
    l.set_credits(1);
    l.declare(Stream::terrain, &[(1, unseen(5.0))], 0.0);
    l.pump(0.0);
    let _ = taken(&mut l);
    l.declare(Stream::terrain, &[(1, unseen(5.0)), (2, unseen(0.0))], 0.0);
    l.pump(0.0);
    let (starts, stops) = taken(&mut l);
    assert!(
        stops.is_empty(),
        "distance says which to start, not which to restart"
    );
    assert!(starts.is_empty());
    l.declare(Stream::terrain, &[(1, unseen(5.0)), (3, seen(9.0))], 0.0);
    l.pump(0.0);
    let (starts, stops) = taken(&mut l);
    assert!(stops.is_empty(), "a class better waits for a free credit");
    assert!(starts.is_empty(), "and starts nothing meanwhile");
}

/// Takes one want a pump, every want ranked alike.
struct Restless;

impl Serve for Restless {
    fn rank(&self, _category: Stream, _subject: u64, order: Order, _enqueue_ms: f64) -> Rank {
        Rank {
            class: 0,
            dist: order.near,
            detail: 0,
        }
    }

    fn intake(&self) -> u32 {
        1
    }

    fn fault(&self, _failure: Failure) -> Fault {
        Fault::Network
    }

    fn backoff(&self, _strikes: u32, _seed: u64) -> Option<f64> {
        Some(0.0)
    }
}

#[test]
fn wants_served_at_rest() {
    let mut l = RequestScheduler::new(Restless, BOUNDS);
    l.set_credits(2);
    let mut wanted: Vec<(u64, Order)> = (0..12).map(|k| (k, seen((k * 7 % 12) as f64))).collect();
    let mut out: Vec<u64> = Vec::new();
    for turn in 0..200 {
        l.declare(Stream::place, &wanted, turn as f64);
        l.pump(turn as f64);
        let (starts, stops) = taken(&mut l);
        out.retain(|k| !stops.contains(k));
        out.extend(starts);
        assert!(out.len() <= 2, "never more out than the credits");
        if let Some(key) = out.first().copied() {
            let (_, id) = l.attempt_of(key).expect("out");
            ok(&mut l, key, turn as f64);
            out.remove(0);
            wanted.retain(|&(s, _)| s != id);
        }
        if wanted.is_empty() {
            return;
        }
    }
    panic!("{} wants never served", wanted.len());
}

#[test]
fn asset_shares_credits() {
    let mut l = scheduler();
    l.set_credits(1);
    l.declare(Stream::terrain, &[(1, seen(0.0))], 0.0);
    l.declare(Stream::asset, &[(7, Order::LAST)], 0.0);
    l.pump(0.0);
    assert_eq!(
        taken(&mut l).0.len(),
        1,
        "one credit, one read — an asset does not go beside the tiles"
    );
    assert_eq!(
        l.stream_counts(Stream::asset),
        (1, 0),
        "and it is what is waiting"
    );
}

#[test]
fn failed_want_reported() {
    let mut l = scheduler();
    let want = [(1u64, seen(0.0))];
    l.declare(Stream::terrain, &want, 0.0);
    l.pump(0.0);
    let key = taken(&mut l).0[0];
    assert!(!l.has_object_failed(Stream::terrain, 1));
    failed_with(&mut l, key, Errno::EBADMSG, 0.0);
    assert!(
        l.has_object_failed(Stream::terrain, 1),
        "its object's failure holds it"
    );
    l.declare(Stream::terrain, &[], 0.0);
    assert!(
        !l.has_object_failed(Stream::terrain, 1),
        "and nothing once it is no longer wanted"
    );
}
