// pxl-core/sys/io/fetch.test.rs

use super::*;
use crate::sys;
use crate::sys::io::demand::Order;
use crate::sys::io::request::{declare, open, pump, set_credits, take_event, Event};

fn arrival(key: u64) -> Arrival {
    Arrival {
        key,
        body: Ok(None),
        read: None,
        net_ms: 0.0,
    }
}

fn answer(key: u64) -> Response {
    Response {
        key,
        stream: Stream::terrain,
        id: 1,
        pkt: None,
        phases: PhaseList::default(),
    }
}

/// What a stopped fetch leaves is let go at once, however many there are; a
/// live one stays for its turn.
#[test]
fn stopped_leave_live_stay() {
    let _g = sys::serial();
    open(|_| 1);
    set_credits(1);
    declare(Stream::terrain, &[(1, Order::FIRST)]);
    pump();
    let live = std::iter::from_fn(take_event)
        .find_map(|e| match e {
            Event::Start(key, ..) => Some(key),
            _ => None,
        })
        .unwrap();
    let stopped = live.wrapping_add(1);
    let q = queue();
    q.arrivals.clear();
    q.answers.clear();
    for _ in 1..2 * CREDITS {
        q.arrivals.push(arrival(stopped));
    }
    q.arrivals.push(arrival(live));
    for _ in 1..CREDITS {
        q.answers.push(answer(stopped));
    }
    q.answers.push(answer(live));
    drop_stopped();
    let q = queue();
    assert_eq!((q.arrivals.len(), q.answers.len()), (1, 1));
    assert_eq!((q.arrivals[0].key, q.answers[0].key), (live, live));
    q.arrivals.clear();
    q.answers.clear();
}
