// xpute-guest/ipc/door.test.rs

use super::*;
use crate::ipc::ring::{ring_bytes, slots_bytes};

const SQ: u32 = 8;
const CQ: u32 = 256;
const SLOT: u32 = 64;
const MOST_SIGNALS: u32 = 64;

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
