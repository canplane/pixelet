// xpute-guest/ipc/door.test.rs

use super::*;
use crate::ipc::ring::{ring_bytes, slots_bytes};

const SQ: u32 = 8;
const CQ: u32 = 256;
const SLOT: u32 = 64;
const MOST_SIGNALS: u32 = 64;
const MOST_COMPLETIONS: u32 = MOST_SIGNALS + 1;

fn door() -> (Door, Box<[u8]>) {
    let sq_desc = 8;
    let cq_desc = sq_desc + ring_bytes(SQ);
    let sq_slot = cq_desc + ring_bytes(CQ);
    let cq_slot = sq_slot + slots_bytes(SQ, SLOT);
    let mut buf = vec![0u8; (cq_slot + slots_bytes(CQ, SLOT)) as usize].into_boxed_slice();
    let layout = DoorLayout {
        slot_bytes: SLOT,
        submission: RingLayout {
            capacity: SQ,
            desc_at: sq_desc,
            slot_at: sq_slot,
        },
        completion: RingLayout {
            capacity: CQ,
            desc_at: cq_desc,
            slot_at: cq_slot,
        },
    };
    // SAFETY: the buffer outlives the door — both are returned together and
    // the caller holds them for the test.
    let d = unsafe { Door::init(buf.as_mut_ptr(), &layout, MOST_SIGNALS, |_, _| {}) };
    (d, buf)
}

#[test]
fn door_a_turn_applies_while_a_command_s_signals_and_reply_fit_and_leaves_the_rest_for_the_next_in_order() {
    let (d, _buf) = door();
    for k in 0..SQ {
        assert_eq!(d.submission().push(k, 1, frame::ACKREQ, None, 0), Errno::OK);
    }
    let mut answered = 0;
    let mut turns = 0;
    loop {
        d.drain(|_, _| {
            for _ in 0..MOST_SIGNALS {
                d.post(0, 2, 0, 0, None);
            }
            Ok(Some(7))
        });
        turns += 1;
        let cq = d.completion();
        while cq.peek() >= 0 {
            let at = cq.peek() as u32;
            if cq.flags(at) & frame::RES != 0 {
                assert_eq!((cq.tag(at), cq.result(at)), (answered, 7), "replies come in the order sent");
            }
            answered += u32::from(cq.flags(at) & frame::RES != 0);
            cq.advance();
        }
        if !d.pending() {
            break;
        }
    }
    let per_turn = CQ / MOST_COMPLETIONS;
    assert_eq!(answered, SQ, "every submission is answered once");
    assert_eq!(turns, SQ.div_ceil(per_turn), "a turn applies as many as the ring has room to answer");
}

#[test]
fn door_a_command_is_not_started_with_room_for_its_signals_but_not_its_reply() {
    let (d, _buf) = door();
    for _ in 0..CQ - MOST_SIGNALS {
        d.post(0, 2, 0, 0, None);
    }
    assert_eq!(d.submission().push(0, 1, frame::ACKREQ, None, 0), Errno::OK);
    d.drain(|_, _| {
        for _ in 0..MOST_SIGNALS {
            d.post(0, 2, 0, 0, None);
        }
        Ok(Some(7))
    });
    assert!(d.pending(), "it waits for the next turn rather than overrunning this one");
}
