// xpute-guest/ipc/door.test.rs

use super::*;
use crate::ipc::ring::{ring_bytes, slots_bytes};
use crate::mem::Memory;

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
    let mem = unsafe { Memory::new(buf.as_mut_ptr().expose_provenance(), buf.len()) };
    // SAFETY: the buffer outlives the door — both are returned together and
    // the caller holds them for the test.
    let (sq, cq) = unsafe { (Ring::init(mem, sq_desc as usize, SQ, sq_slot, SLOT), Ring::init(mem, cq_desc as usize, CQ, cq_slot, SLOT)) };
    (Door::new(sq, cq, MOST_SIGNALS), buf)
}

#[test]
fn door_a_command_is_not_started_with_room_for_its_signals_but_not_its_reply() {
    let (d, _buf) = door();
    for _ in 0..CQ - MOST_SIGNALS {
        d.post(0, 2, 0, 0);
    }
    assert_eq!(d.sq().push(0, 1, frame::ACKREQ, 0, None), Errno::OK);
    d.drain(|_, _| {
        for _ in 0..MOST_SIGNALS {
            d.post(0, 2, 0, 0);
        }
        Ok(Some(7))
    });
    assert!(d.pending(), "it waits for the next turn rather than overrunning this one");
}

#[test]
fn door_keeps_the_rings_peaks_until_they_are_taken_and_starts_again_from_zero() {
    let (d, _buf) = door();
    // Three commands start before the reserve runs out, so five of eight wait.
    for _ in 0..CQ - (MOST_SIGNALS + 1) - 2 {
        d.post(0, 2, 0, 0);
    }
    for _ in 0..SQ {
        assert_eq!(d.sq().push(0, 1, frame::ACKREQ, 0, None), Errno::OK);
    }
    assert_eq!(d.drain(|_, _| Ok(Some(0))), 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: CQ - MOST_SIGNALS,
            sq_peak: SQ,
            left_peak: SQ - 3
        }
    );
    assert_eq!(d.take_stat(), DoorStat::default(), "taking it starts the next from zero");

    while d.cq().peek().is_some() {
        d.cq().advance();
    }
    assert_eq!(d.drain(|_, _| Ok(Some(0))), SQ - 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: SQ - 3,
            sq_peak: SQ - 3,
            left_peak: 0
        }
    );
}
