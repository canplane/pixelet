// pxl-core/sys/io/request.test.rs
//
// Tests of what the request scheduler keeps, under the real policy and a
// clock the test sets.

use super::*;
use core::cell::Cell;
use xpute_core::clock;
use xpute_core::mem::heap::HeapStat;
use xpute_kit::status::errno::Errno;

use crate::sys::boot::test::{heap_release, set_counted_heap, set_heap_stat};
use crate::sys::io::param::RETRY_ATTEMPTS;

thread_local! {
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn at(t: f64) {
    NOW.set(t);
    clock::set(|| NOW.get());
}

/// Opens the scheduler with room for every test's wants in each stream, and
/// two credits.
fn begin() {
    at(0.0);
    open(|_| 64);
    set_credits(2);
}

fn seen(near: f64) -> Order {
    Order { seen: true, near }
}

/// An order that ranks after every seen want, however near.
fn unseen(near: f64) -> Order {
    Order { seen: false, near }
}

/// Declares and starts one terrain want, id 1, and returns its key.
fn one_out() -> u64 {
    begin();
    declare(Stream::terrain, &[(1, seen(0.0))]);
    pump();
    let (starts, _) = take_events();
    assert_eq!(starts.len(), 1);
    starts[0]
}

/// Takes every pending event, and returns the keys started and the keys
/// stopped.
fn take_events() -> (Vec<u64>, Vec<u64>) {
    let (mut starts, mut stops) = (Vec::new(), Vec::new());
    while let Some(event) = take_event() {
        match event {
            Event::Start(key, _, _) => starts.push(key),
            Event::Stop(key) => stops.push(key),
            Event::End(..) => {}
        }
    }
    (starts, stops)
}

/// How many of `stream`'s wants wait for a credit, and how many are out on
/// one.
fn cnts(stream: Stream) -> (usize, usize) {
    let s = sched();
    let waiting = s.wants[stream as usize]
        .iter()
        .filter(|w| !attempting(s, stream, w.id))
        .count();
    let out = s.slots.iter().flat_map(|c| c.out);
    (waiting, out.filter(|a| a.stream == stream).count())
}

fn until() -> f64 {
    sched().outage.until
}

fn ok(key: u64) {
    settle(key, Exit::Ok, None);
}

fn failed(key: u64, errnum: Errno) {
    let fail = Failure {
        errnum,
        detail: 0,
        at: None,
    };
    settle(key, Exit::Failed(fail), None);
}

#[test]
fn same_set_no_change() {
    crate::on_core(|| {
        one_out();
        for _ in 0..5 {
            at(0.0);
            declare(Stream::terrain, &[(1, seen(0.0))]);
            pump();
            assert!(
                take_events().0.is_empty(),
                "already out, so nothing is started again"
            );
        }
    });
}

#[test]
fn reset_stops_what_is_out() {
    crate::on_core(|| {
        let key = one_out();
        declare(Stream::terrain, &[(1, seen(0.0)), (2, seen(1.0))]);
        pump();
        reset();
        let (starts, stops) = take_events();
        assert!(starts.is_empty(), "a start outlived the reset");
        assert_eq!(stops.len(), 2);
        assert!(stops.contains(&key));
        assert!(attempt_of(key).is_none());
        assert_eq!(cnts(Stream::terrain), (0, 0));
    });
}

#[test]
fn answer_asked_once() {
    crate::on_core(|| {
        let key = one_out();
        ok(key);
        at(0.0);
        pump();
        let (starts, stops) = take_events();
        assert!(starts.is_empty(), "an answered want was asked again");
        assert!(stops.is_empty());
        assert_eq!(cnts(Stream::terrain), (0, 0));
        at(1.0);
        declare(Stream::terrain, &[(1, seen(0.0))]);
        pump();
        assert_eq!(take_events().0.len(), 1);
    });
}

#[test]
fn failure_backs_off() {
    crate::on_core(|| {
        let key = one_out();
        failed(key, Errno::ENETUNREACH);
        // A failure does not withdraw the want.
        for t in 0..5 {
            at(until() * f64::from(t) / 5.0);
            declare(Stream::terrain, &[(1, seen(0.0))]);
            pump();
            assert!(
                take_events().0.is_empty(),
                "held back, not re-asked every turn"
            );
        }
        assert_eq!(cnts(Stream::terrain), (1, 0), "wanted, nothing out");
        at(until());
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "and tried again once the wait is over"
        );
    });
}

#[test]
fn failed_object_dropped() {
    crate::on_core(|| {
        begin();
        let want = [(1u64, seen(0.0)), (2, seen(1.0))];
        at(0.0);
        declare(Stream::terrain, &want);
        pump();
        let out = take_events().0;
        at(0.0);
        failed(out[0], Errno::ENXIO);
        ok(out[1]);
        for t in 0..5 {
            at(t as f64 * 1_000.0);
            declare(Stream::terrain, &want[..1]);
            pump();
            assert!(
                take_events().0.is_empty(),
                "a missing object is not asked again"
            );
        }
        at(5_000.0);
        declare(Stream::terrain, &[(3, seen(0.0))]);
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "and the server is not held for it"
        );
    });
}

#[test]
fn heap_refusal_waits_release() {
    crate::on_core(|| {
        let key = one_out();
        set_counted_heap();
        at(0.0);
        settle(key, Exit::Refused(Refusal::now()), None);
        for t in 0..5 {
            at(t as f64 * 10_000.0);
            declare(Stream::terrain, &[(1, seen(0.0))]);
            pump();
            assert!(take_events().0.is_empty(), "no time ends a heap's refusal");
        }
        let both = [(1, seen(0.0)), (2, seen(0.0))];
        at(50_000.0);
        declare(Stream::terrain, &both);
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "and the server is not held for it"
        );
        heap_release();
        at(50_000.0);
        declare(Stream::terrain, &both);
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "asked again once the heap gives some back"
        );
        set_heap_stat(HeapStat::default);
    });
}

#[test]
fn early_release_not_rewaited() {
    crate::on_core(|| {
        let key = one_out();
        set_counted_heap();
        // The heap refuses, then gives room back before the refusal is settled.
        let refused = Refusal::now();
        heap_release();
        at(0.0);
        settle(key, Exit::Refused(refused), None);
        at(0.0);
        declare(Stream::terrain, &[(1, seen(0.0))]);
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "the release after the refusal already ends its wait"
        );
        set_heap_stat(HeapStat::default);
    });
}

#[test]
fn failing_server_probe() {
    crate::on_core(|| {
        begin();
        let want: Vec<(u64, Order)> = (1..5).map(|k| (k, seen(k as f64))).collect();
        declare(Stream::terrain, &want);
        pump();
        let out = take_events().0;
        failed(out[0], Errno::EACCES);
        at(until() / 2.0);
        pump();
        assert!(
            take_events().0.is_empty(),
            "held: the credit freed starts nothing"
        );
        at(until());
        pump();
        assert!(
            take_events().0.is_empty(),
            "one out already: the other attempt is the probe"
        );
        failed(out[1], Errno::EACCES);
        assert_eq!(
            sched().outage.fail_cnt,
            1,
            "out before the failure, so the same outage"
        );
        for n in 2..=RETRY_ATTEMPTS + 1 {
            at(until());
            pump();
            let probe = take_events().0;
            assert_eq!(probe.len(), 1, "one probe after the wait");
            failed(probe[0], Errno::EACCES);
            assert_eq!(sched().outage.fail_cnt, n);
        }
        assert!(given_up(), "past the last wait");
        at(until() + 1e9);
        pump();
        assert!(take_events().0.is_empty(), "nothing is asked once given up");
    });
}

#[test]
fn ended_hold_asks_no_turn() {
    crate::on_core(|| {
        let key = one_out();
        failed(key, Errno::ENETUNREACH);
        declare(Stream::terrain, &[]);
        pump();
        assert!(wake() >= 0.0, "the hold's end is asked for");
        at(until());
        pump();
        assert!(wake() < 0.0, "and nothing after it");
    });
}

/// What a turn's end asks for: the end of the hold, or none.
fn wake() -> f64 {
    if let Some(at) = next_pump_at(now()) {
        sched::wake_at(at);
    }
    sched::turn().end()
}

/// A probe that fails after the turn's pump still brings a pump that asks for
/// the end of the new hold, and a probe that is out asks for no turn.
#[test]
fn failed_probe_keeps_retrying() {
    crate::on_core(|| {
        let key = one_out();
        failed(key, Errno::ENETUNREACH);
        assert_eq!(wake(), 0.0, "a pump is asked for");
        declare(Stream::terrain, &[(1, seen(0.0))]);
        pump();
        assert!(wake() > 0.0, "which asks for the hold's end");
        at(until());
        pump();
        let (starts, _) = take_events();
        assert_eq!(starts.len(), 1, "the probe");
        assert!(wake() < 0.0, "out, it asks for no turn");
        failed(starts[0], Errno::ENETUNREACH);
        assert_eq!(wake(), 0.0, "and failing, one more pump");
    });
}

#[test]
fn outage_counts_once() {
    crate::on_core(|| {
        begin();
        at(0.0);
        declare(Stream::terrain, &[(1, seen(0.0)), (2, seen(1.0))]);
        pump();
        for key in take_events().0 {
            at(10.0);
            failed(key, Errno::EACCES);
        }
        assert_eq!(sched().outage.fail_cnt, 1, "one outage, one wait");
        assert!(!given_up());
    });
}

#[test]
fn cur_demand_said() {
    crate::on_core(|| {
        begin();
        at(0.0);
        declare(Stream::terrain, &[(1, unseen(0.0)), (2, unseen(1.0))]);
        pump();
        let out = take_events().0;
        assert_eq!(out.len(), 2);
        at(0.0);
        declare(Stream::terrain, &[(3, seen(0.0))]);
        assert_eq!(
            cnts(Stream::terrain).0,
            1,
            "the new want is representable with both credits spent"
        );
        at(0.0);
        pump();
        // What the camera left is stopped in the turn it leaves, and the freed
        // credits start what is wanted now in the same turn.
        let (starts, stops) = take_events();
        assert_eq!(stops.len(), 2, "every obsolete attempt is told to stop");
        assert_eq!(starts.len(), 1, "and the current want starts at once");
        ok(out[0]);
        assert_eq!(
            cnts(Stream::terrain),
            (0, 1),
            "the late report neither frees nor ends anything"
        );
    });
}

#[test]
fn running_attempt_kept() {
    crate::on_core(|| {
        begin();
        set_credits(1);
        declare(Stream::terrain, &[(1, unseen(5.0))]);
        pump();
        let _ = take_events();
        declare(Stream::terrain, &[(1, unseen(5.0)), (2, unseen(0.0))]);
        pump();
        let (starts, stops) = take_events();
        assert!(
            stops.is_empty(),
            "distance says which to start, not which to restart"
        );
        assert!(starts.is_empty());
        at(0.0);
        declare(Stream::terrain, &[(1, unseen(5.0)), (3, seen(9.0))]);
        pump();
        let (starts, stops) = take_events();
        assert!(stops.is_empty(), "a class better waits for a free credit");
        assert!(starts.is_empty(), "and starts nothing meanwhile");
    });
}

#[test]
fn wants_served_at_rest() {
    crate::on_core(|| {
        begin();
        let mut wanted: Vec<(u64, Order)> =
            (1..13).map(|k| (k, seen((k * 7 % 12) as f64))).collect();
        let mut out: Vec<u64> = Vec::new();
        for turn in 0..200 {
            at(turn as f64);
            declare(Stream::place, &wanted);
            pump();
            let (starts, stops) = take_events();
            out.retain(|k| !stops.contains(k));
            out.extend(starts);
            assert!(out.len() <= 2, "never more out than the credits");
            if let Some(key) = out.first().copied() {
                let (_, id) = attempt_of(key).expect("out");
                ok(key);
                out.remove(0);
                wanted.retain(|&(s, _)| s != id);
            }
            if wanted.is_empty() {
                return;
            }
        }
        panic!("{} wants never served", wanted.len());
    });
}

#[test]
fn asset_shares_credits() {
    crate::on_core(|| {
        begin();
        set_credits(1);
        at(0.0);
        declare(Stream::terrain, &[(1, seen(0.0))]);
        at(0.0);
        declare(Stream::asset, &[(7, Order::LAST)]);
        pump();
        assert_eq!(
            take_events().0.len(),
            1,
            "one credit, one read — an asset does not go beside the tiles"
        );
        assert_eq!(cnts(Stream::asset), (1, 0), "and it is what is waiting");
    });
}

#[test]
fn failed_want_reported() {
    crate::on_core(|| {
        let key = one_out();
        assert!(!has_object_failed(Stream::terrain, 1));
        at(0.0);
        failed(key, Errno::EBADMSG);
        assert!(
            has_object_failed(Stream::terrain, 1),
            "its object's failure holds it"
        );
        at(0.0);
        declare(Stream::terrain, &[]);
        assert!(
            !has_object_failed(Stream::terrain, 1),
            "and nothing once it is no longer wanted"
        );
    });
}
