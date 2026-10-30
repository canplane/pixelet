// xpute-guest/ipc/door.test.rs

use super::*;
use crate::ipc::ring::{ring_bytes, slots_bytes};
use crate::mem::Memory;

const SQ: u32 = 8;
const CQ: u32 = 256;
const SLOT: u32 = 64;
const MOST_EVENTS: u32 = 64;
/// A program's port, sending the program's commands.
const PROGRAM: u32 = 1 << frame::PORT_SHIFT;

fn door() -> (Door, Box<[u8]>) {
    let sq_desc = 8;
    let cq_desc = sq_desc + ring_bytes(SQ);
    let sq_slot = cq_desc + ring_bytes(CQ);
    let cq_slot = sq_slot + slots_bytes(SQ, SLOT);
    let mut buf = vec![0u8; (cq_slot + slots_bytes(CQ, SLOT)) as usize].into_boxed_slice();
    let mem = unsafe { Memory::new(buf.as_mut_ptr().expose_provenance(), buf.len()) };
    // SAFETY: the buffer outlives the door — both are returned together and
    // the caller holds them for the test.
    let (sq, cq) = unsafe {
        (
            Ring::init(mem, sq_desc as usize, SQ, sq_slot, SLOT),
            Ring::init(mem, cq_desc as usize, CQ, cq_slot, SLOT),
        )
    };
    (Door::new(sq, cq, MOST_EVENTS), buf)
}

#[test]
fn command_needs_reply_room() {
    let (d, _buf) = door();
    for _ in 0..CQ - MOST_EVENTS {
        d.post(0, 2, 0, 0);
    }
    assert_eq!(d.sq().push(PROGRAM, 1, frame::ACKREQ, 0, None), Errno::OK);
    d.drain(|_| {
        for _ in 0..MOST_EVENTS {
            d.post(0, 2, 0, 0);
        }
        Ok(Some(7))
    });
    assert!(
        d.pending(),
        "it waits for the next turn rather than overrunning this one"
    );
}

#[test]
fn ring_peaks() {
    let (d, _buf) = door();
    // Three commands start before the reserve runs out, so five of eight wait.
    for _ in 0..CQ - (MOST_EVENTS + 1) - 2 {
        d.post(0, 2, 0, 0);
    }
    for _ in 0..SQ {
        assert_eq!(d.sq().push(PROGRAM, 1, frame::ACKREQ, 0, None), Errno::OK);
    }
    assert_eq!(d.drain(|_| Ok(Some(0))), 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: CQ - MOST_EVENTS,
            sq_peak: SQ,
            left_peak: SQ - 3
        }
    );
    assert_eq!(
        d.take_stat(),
        DoorStat::default(),
        "taking it starts the next from zero"
    );

    while d.cq().peek().is_some() {
        d.cq().advance();
    }
    assert_eq!(d.drain(|_| Ok(Some(0))), SQ - 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: SQ - 3,
            sq_peak: SQ - 3,
            left_peak: 0
        }
    );
}

/// SYS_OPENED's packet: the descriptor, its result and no detail.
fn sys_opened(fd: u32, res: i32) -> Vec<u8> {
    let mut buf = vec![0u8; SLOT as usize];
    let mut w = PacketWriter::new(&mut buf, 3);
    w.u32(Some(fd)).i32(Some(res)).u32(Some(0));
    let len = w.fitted().expect("a packet that fits a slot") as usize;
    buf.truncate(len);
    buf
}

#[test]
fn door_answers_nop() {
    let (d, _buf) = door();
    assert_eq!(
        d.sq().push(4, Command::NOP as u32, frame::ACKREQ, 0, None),
        Errno::OK
    );
    d.drain(|_| unreachable!("the program is handed no NOP"));
    let f = d.cq().peek().expect("a reply");
    assert_eq!(
        (f.tag, f.cmd, f.flags & frame::RES, f.result),
        (4, Command::NOP as u32, frame::RES, 0)
    );
}

#[test]
fn open_completion_handed_over() {
    let (d, _buf) = door();
    let pkt = sys_opened(3, 9);
    assert_eq!(
        d.sq().push(0, Command::SYS_OPENED as u32, 0, 0, Some(&pkt)),
        Errno::OK
    );
    let mut got = None;
    d.drain(|submission| match submission {
        Submission::Opened(opened) => {
            got = Some((opened.fd, opened.res));
            Ok(Some(0))
        }
        Submission::Program(..) => unreachable!("an open's completion is not the program's frame"),
    });
    assert_eq!(got, Some((3, 9)));
}

#[test]
fn program_frame_numbered_nop_handed_over() {
    let (d, _buf) = door();
    let tag = PROGRAM | 6;
    assert_eq!(
        d.sq()
            .push(tag, Command::NOP as u32, frame::ACKREQ, 0, None),
        Errno::OK
    );
    let mut got = None;
    d.drain(|submission| match submission {
        Submission::Program(cmd, _) => {
            got = Some(cmd);
            Ok(Some(1))
        }
        Submission::Opened(_) => unreachable!("port 1 is not the host"),
    });
    assert_eq!(got, Some(Command::NOP as u32));
    let f = d.cq().peek().expect("a reply");
    assert_eq!(
        (f.tag, f.result),
        (tag, 1),
        "the reply goes back to its sender"
    );
}

#[test]
fn host_frame_unknown_refused() {
    let (d, _buf) = door();
    assert_eq!(d.sq().push(5, 0x1001, 0, 0, None), Errno::OK);
    d.drain(|_| unreachable!("the host's command is none of the program's"));
    let f = d.cq().peek().expect("a failure's reply");
    assert_eq!((f.tag, f.result), (5, Errno::ENOSYS as i32));
}
