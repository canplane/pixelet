// pxl-core/sys/io/request.test.rs
//
// Tests of what the request scheduler keeps, under the real policy and a
// clock the test sets.

use super::*;
use xpute_kit::status::errno::Errno;

use crate::sys::host::set_clock;
use crate::sys::io::failure::Failure;
use crate::sys::io::param::RETRY_ATTEMPT_CNT;

/// Opens the scheduler with room for every test's wants in each stream.
fn begin() {
    set_clock(0.0);
    open(|_| 64);
}

/// Opens the scheduler and puts a far terrain want out on every credit,
/// ids from 100 on. Returns the wants.
fn all_out() -> Vec<(u64, Order)> {
    begin();
    let far: Vec<(u64, Order)> = (0..SLOT_CNT as u64)
        .map(|k| (100 + k, unseen(50.0 + k as f64)))
        .collect();
    declare(Stream::Terrain, &far);
    pump();
    assert_eq!(take_events().0.len(), SLOT_CNT);
    far
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
    declare(Stream::Terrain, &[(1, seen(0.0))]);
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

/// The read under `key` ends at the host with an answer.
fn ok(key: u64) {
    end_read(key);
    settle(key, Exit::Ok, None);
}

fn failed(key: u64, errnum: Errno) {
    let fail = Failure::from(errnum);
    end_read(key);
    settle(key, Exit::Failed(fail), None);
}

#[test]
fn same_set_no_change() {
    one_out();
    for _ in 0..5 {
        set_clock(0.0);
        declare(Stream::Terrain, &[(1, seen(0.0))]);
        pump();
        assert!(
            take_events().0.is_empty(),
            "already out, so nothing is started again"
        );
    }
}

#[test]
fn answer_asked_once() {
    let key = one_out();
    ok(key);
    set_clock(0.0);
    pump();
    let (starts, stops) = take_events();
    assert!(starts.is_empty(), "an answered want was asked again");
    assert!(stops.is_empty());
    assert_eq!(cnts(Stream::Terrain), (0, 0));
    set_clock(1.0);
    declare(Stream::Terrain, &[(1, seen(0.0))]);
    pump();
    assert_eq!(take_events().0.len(), 1);
}

#[test]
fn failure_backs_off() {
    let key = one_out();
    failed(key, Errno::ENETUNREACH);
    // A failure does not withdraw the want.
    for t in 0..5 {
        set_clock(until() * f64::from(t) / 5.0);
        declare(Stream::Terrain, &[(1, seen(0.0))]);
        pump();
        assert!(
            take_events().0.is_empty(),
            "held back, not re-asked every frame"
        );
    }
    assert_eq!(cnts(Stream::Terrain), (1, 0), "wanted, nothing out");
    set_clock(until());
    pump();
    assert_eq!(
        take_events().0.len(),
        1,
        "and tried again once the wait is over"
    );
}

#[test]
fn failed_object_dropped() {
    begin();
    let want = [(1u64, seen(0.0)), (2, seen(1.0))];
    set_clock(0.0);
    declare(Stream::Terrain, &want);
    pump();
    let out = take_events().0;
    set_clock(0.0);
    failed(out[0], Errno::ENXIO);
    ok(out[1]);
    for t in 0..5 {
        set_clock(t as f64 * 1_000.0);
        declare(Stream::Terrain, &want[..1]);
        pump();
        assert!(
            take_events().0.is_empty(),
            "a missing object is not asked again"
        );
    }
    set_clock(5_000.0);
    declare(Stream::Terrain, &[(3, seen(0.0))]);
    pump();
    assert_eq!(
        take_events().0.len(),
        1,
        "and the server is not held for it"
    );
}

#[test]
fn failing_server_probe() {
    begin();
    let want: Vec<(u64, Order)> = (1..5).map(|k| (k, seen(k as f64))).collect();
    declare(Stream::Terrain, &want);
    pump();
    let out = take_events().0;
    failed(out[0], Errno::EACCES);
    set_clock(until() / 2.0);
    pump();
    assert!(
        take_events().0.is_empty(),
        "held: the credit freed starts nothing"
    );
    set_clock(until());
    pump();
    assert!(
        take_events().0.is_empty(),
        "others are out already: they are the probe"
    );
    for &key in &out[1..] {
        failed(key, Errno::EACCES);
    }
    assert_eq!(
        sched().outage.fail_cnt,
        1,
        "out before the failure, so the same outage"
    );
    for n in 2..=RETRY_ATTEMPT_CNT + 1 {
        set_clock(until());
        pump();
        let probe = take_events().0;
        assert_eq!(probe.len(), 1, "one probe after the wait");
        failed(probe[0], Errno::EACCES);
        assert_eq!(sched().outage.fail_cnt, n);
    }
    assert!(given_up(), "past the last wait");
    set_clock(until() + 1e9);
    pump();
    assert!(take_events().0.is_empty(), "nothing is asked once given up");
}

#[test]
fn outage_counts_once() {
    begin();
    set_clock(0.0);
    declare(Stream::Terrain, &[(1, seen(0.0)), (2, seen(1.0))]);
    pump();
    for key in take_events().0 {
        set_clock(10.0);
        failed(key, Errno::EACCES);
    }
    assert_eq!(sched().outage.fail_cnt, 1, "one outage, one wait");
    assert!(!given_up());
}

#[test]
fn cur_demand_said() {
    begin();
    declare(Stream::Terrain, &[(1, unseen(0.0)), (2, unseen(1.0))]);
    pump();
    let out = take_events().0;
    assert_eq!(out.len(), 2);
    declare(Stream::Terrain, &[(3, seen(0.0))]);
    pump();
    // What the camera left is stopped in the frame it leaves, and what is
    // wanted now starts in the same frame on a credit that is free.
    let (starts, stops) = take_events();
    assert_eq!(stops.len(), 2, "every obsolete attempt is told to stop");
    assert_eq!(starts.len(), 1, "and the current want starts at once");
    ok(out[0]);
    assert_eq!(
        cnts(Stream::Terrain),
        (0, 1),
        "the late report ends no attempt"
    );
}

/// A read stopped holds its credit until the host lets it go: with every
/// credit's read stopped, what is wanted now starts only as each read ends.
#[test]
fn a_stopped_read_holds_its_credit() {
    all_out();
    set_clock(0.0);
    declare(Stream::Terrain, &[(1, seen(0.0)), (2, seen(1.0))]);
    pump();
    let (starts, stops) = take_events();
    assert_eq!(stops.len(), SLOT_CNT, "every obsolete attempt is stopped");
    assert!(starts.is_empty(), "their reads are still at the host");
    end_read(stops[0]);
    pump();
    assert_eq!(take_events().0.len(), 1, "one read ended, one starts");
    end_read(stops[0]);
    pump();
    assert!(take_events().0.is_empty(), "a key is spent once");
    end_read(stops[1]);
    pump();
    assert_eq!(take_events().0.len(), 1);
}

#[test]
fn running_attempt_kept() {
    let out = all_out();
    let with = |more: (u64, Order)| {
        let mut w = vec![more];
        w.extend(&out);
        w
    };
    declare(Stream::Terrain, &with((2, unseen(0.0))));
    pump();
    let (starts, stops) = take_events();
    assert!(
        stops.is_empty(),
        "distance says which to start, not which to restart"
    );
    assert!(starts.is_empty());
    set_clock(0.0);
    declare(Stream::Terrain, &with((3, seen(9.0))));
    pump();
    let (starts, stops) = take_events();
    assert!(stops.is_empty(), "a class better waits for a free credit");
    assert!(starts.is_empty(), "and starts nothing meanwhile");
}

#[test]
fn wants_served_at_rest() {
    begin();
    let mut wanted: Vec<(u64, Order)> = (1..13).map(|k| (k, seen((k * 7 % 12) as f64))).collect();
    let mut out: Vec<u64> = Vec::new();
    for t in 0..200 {
        set_clock(t as f64);
        declare(Stream::Place, &wanted);
        pump();
        let (starts, stops) = take_events();
        out.retain(|k| !stops.contains(k));
        out.extend(starts);
        assert!(out.len() <= SLOT_CNT, "never more out than the credits");
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
}

#[test]
fn asset_shares_credits() {
    begin();
    set_clock(0.0);
    let tiles: Vec<(u64, Order)> = (1..=SLOT_CNT as u64).map(|k| (k, seen(k as f64))).collect();
    declare(Stream::Terrain, &tiles);
    set_clock(0.0);
    declare(Stream::Asset, &[(7, Order::LAST)]);
    pump();
    assert_eq!(
        take_events().0.len(),
        SLOT_CNT,
        "a credit, a read: an asset does not go beside the tiles"
    );
    assert_eq!(cnts(Stream::Asset), (1, 0), "and it is what is waiting");
}

#[test]
fn failed_want_reported() {
    let key = one_out();
    assert!(!has_object_failed(Stream::Terrain, 1));
    set_clock(0.0);
    failed(key, Errno::EBADMSG);
    assert!(
        has_object_failed(Stream::Terrain, 1),
        "its object's failure holds it"
    );
    set_clock(0.0);
    declare(Stream::Terrain, &[]);
    assert!(
        !has_object_failed(Stream::Terrain, 1),
        "and nothing once it is no longer wanted"
    );
}
