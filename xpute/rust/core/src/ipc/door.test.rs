// xpute-core/ipc/door.test.rs

use super::*;
use crate::ipc::ring::{ring_bytes, slots_bytes};
use crate::mem::Memory;

const SQ: u32 = 8;
const CQ: u32 = 256;
const SLOT: u32 = 64;
const MOST_FRAMES: u32 = 64;
/// A command of the program's own, past xpute's majors.
const PROGRAM: u32 = frame::PROGRAM_MAJOR << 8 | 1;

fn door() -> (Door, Box<[u8]>) {
    door_of(SLOT)
}

fn door_of(slot: u32) -> (Door, Box<[u8]>) {
    let sq_desc = 8;
    let cq_desc = sq_desc + ring_bytes(SQ);
    let sq_slot = cq_desc + ring_bytes(CQ);
    let cq_slot = sq_slot + slots_bytes(SQ, slot);
    let mut buf = vec![0u8; (cq_slot + slots_bytes(CQ, slot)) as usize].into_boxed_slice();
    let mem = unsafe { Memory::new(buf.as_mut_ptr().expose_provenance(), buf.len()) };
    // SAFETY: the buffer outlives the door — both are returned together and
    // the caller holds them for the test.
    let (sq, cq) = unsafe {
        (
            Ring::init(mem, sq_desc as usize, SQ, sq_slot, slot),
            Ring::init(mem, cq_desc as usize, CQ, cq_slot, slot),
        )
    };
    (Door::new(sq, cq, MOST_FRAMES), buf)
}

#[test]
fn command_needs_reply_room() {
    let (d, _buf) = door();
    for _ in 0..CQ - MOST_FRAMES {
        d.post(0, 2, 0, 0);
    }
    assert_eq!(d.sq().push(0, PROGRAM, frame::ACKREQ, 0, None), Errno::OK);
    d.receive(|_| {
        for _ in 0..MOST_FRAMES {
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
    for _ in 0..CQ - (MOST_FRAMES + 2) - 2 {
        d.post(0, 2, 0, 0);
    }
    for _ in 0..SQ {
        assert_eq!(d.sq().push(0, PROGRAM, frame::ACKREQ, 0, None), Errno::OK);
    }
    assert_eq!(d.receive(|_| Ok(Some(0))), 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: CQ - MOST_FRAMES - 1,
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
    assert_eq!(d.receive(|_| Ok(Some(0))), SQ - 3);
    assert_eq!(
        d.take_stat(),
        DoorStat {
            cq_peak: SQ - 3,
            sq_peak: SQ - 3,
            left_peak: 0
        }
    );
}

/// FS_OPENAT's completion's packet: no detail.
fn openat_reply() -> Vec<u8> {
    let mut buf = vec![0u8; SLOT as usize];
    let mut w = PacketWriter::new(&mut buf, 1);
    w.u32(Some(0));
    let len = w.fit_len().expect("a packet that fits a slot") as usize;
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
    d.receive(|_| unreachable!("the program is handed no NOP"));
    let f = d.cq().peek().expect("a reply");
    assert_eq!(
        (f.tag, f.cmd, f.flags & frame::RES, f.result),
        (4, Command::NOP as u32, frame::RES, 0)
    );
}

#[test]
fn open_completion_handed_over() {
    let (d, _buf) = door();
    let pkt = openat_reply();
    assert_eq!(
        d.sq()
            .push(3, fs::Command::FS_OPENAT as u32, frame::RES, 9, Some(&pkt)),
        Errno::OK
    );
    let mut got = None;
    d.receive(|submission| match submission {
        Submission::Opened(opened) => {
            got = Some((opened.fd, opened.res));
            Ok(Some(0))
        }
        Submission::Life(_)
        | Submission::Turn(_)
        | Submission::GpuReady
        | Submission::Surface(_)
        | Submission::Closed(_)
        | Submission::Changed(_)
        | Submission::Program(..) => {
            unreachable!("an open's completion is not the program's frame")
        }
    });
    assert_eq!(got, Some((3, 9)));
    assert!(
        d.cq().peek().is_none(),
        "a completion is answered by nothing"
    );
}

#[test]
fn open_and_close_ask_for_completion() {
    let (d, _buf) = door_of(256);
    d.openat(5, 1, 0, "a");
    d.close(5);
    for cmd in [fs::Command::FS_OPENAT, fs::Command::FS_CLOSE] {
        let f = d.cq().peek().expect("a request");
        assert_eq!(
            (f.tag, f.cmd, f.flags & frame::ACKREQ),
            (5, cmd as u32, frame::ACKREQ),
            "tagged with its descriptor"
        );
        d.cq().advance();
    }
}

/// A write's bytes are the runtime's until the descriptor's close is
/// completed, however far the runtime's head has gone.
#[test]
fn write_kept_until_close_completed() {
    let (d, _buf) = door();
    d.write(5, vec![1, 2, 3]);
    d.close(5);
    while d.cq().peek().is_some() {
        d.cq().advance();
    }
    d.rise();
    assert_eq!(d.writes.borrow().len(), 1, "the head passing frees nothing");
    assert_eq!(
        d.sq()
            .push(5, fs::Command::FS_CLOSE as u32, frame::RES, 0, None),
        Errno::OK
    );
    let mut closed = None;
    d.receive(|submission| {
        if let Submission::Closed(c) = submission {
            closed = Some((c.fd, c.res));
        }
        Ok(Some(0))
    });
    assert_eq!(closed, Some((5, 0)));
    assert!(
        d.writes.borrow().is_empty(),
        "the close's completion frees it"
    );
}

#[test]
fn program_command_handed_over() {
    let (d, _buf) = door();
    let tag = 6;
    assert_eq!(d.sq().push(tag, PROGRAM, frame::ACKREQ, 0, None), Errno::OK);
    let mut got = None;
    d.receive(|submission| match submission {
        Submission::Program(cmd, _) => {
            got = Some(cmd);
            Ok(Some(1))
        }
        Submission::Life(_)
        | Submission::Turn(_)
        | Submission::GpuReady
        | Submission::Surface(_)
        | Submission::Opened(_)
        | Submission::Closed(_)
        | Submission::Changed(_) => {
            unreachable!("a command past xpute's majors is the program's")
        }
    });
    assert_eq!(got, Some(PROGRAM));
    let f = d.cq().peek().expect("a reply");
    assert_eq!(
        (f.tag, f.result),
        (tag, 1),
        "the reply goes back to its sender"
    );
}

#[test]
fn xpute_number_unknown_refused() {
    let (d, _buf) = door();
    assert_eq!(d.sq().push(5, 0x0f01, 0, 0, None), Errno::OK);
    d.receive(|_| unreachable!("a number under xpute's majors is none of the program's"));
    let f = d.cq().peek().expect("a failure's reply");
    assert_eq!((f.tag, f.result), (5, Errno::ENOSYS as i32));
}

/// What `call` sends, an import's frame with no fields.
struct Ping;

impl Import for Ping {
    const NUMBER: u32 = 0x1201;
    const FIELDS: u32 = 0;
    fn write(&self, _: &mut PacketWriter) {}
}

#[test]
fn call_publishes_nothing() {
    let (d, _buf) = door();
    d.send(&Ping);
    // No runtime's `call` is installed in a native test, so the call is refused.
    assert_eq!(d.call(&Ping), Err(Errno::ENOSYS));
    assert_eq!(d.cq().len(), 1, "the call's frame is laid, not published");
}

#[test]
fn abort_posts_the_report_after_what_was_published() {
    let (d, _buf) = door_of(256);
    d.send(&Ping);
    d.close(3);
    d.abort(Errno::EINVAL, core::panic::Location::caller(), 1, 2);
    assert_eq!(d.cq().len(), 3, "no published frame is taken back");
    d.cq().advance();
    d.cq().advance();
    let f = d.cq().peek().expect("the report");
    assert_eq!(f.tag, 0);
    assert_eq!(f.cmd, crate::abi::sys::Command::SYS_ABORT as u32);
}

#[test]
fn a_full_ring_keeps_the_report_slot() {
    let (d, _buf) = door_of(256);
    while d.cq().len() + 1 < d.cq().cap {
        d.send(&Ping);
    }
    assert_eq!(
        d.call(&Ping),
        Err(Errno::ENOSPC),
        "a call takes no slot of the report's"
    );
    d.abort(Errno::EINVAL, core::panic::Location::caller(), 1, 2);
    assert_eq!(d.cq().len(), d.cq().cap, "the report took the last slot");
}

/// A reading dropped before the head has passed it leaves its buffer with
/// the door, which lets it go at the first rise after.
#[test]
fn dropped_reading_kept_until_consumed() {
    let (d, _buf) = door();
    drop(d.read_buf(3, vec![0; 5]));
    d.rise();
    assert_eq!(d.reads.borrow().len(), 1, "the runtime may still fill it");
    while d.cq().peek().is_some() {
        d.cq().advance();
    }
    d.rise();
    assert!(d.reads.borrow().is_empty());
}
