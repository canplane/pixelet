// xpute-core/io/door.test.rs

use core::cell::{Cell, RefCell};
use std::collections::VecDeque;

use super::*;
use crate::abi::sys::Command;
use crate::host::Host;
use crate::io::fs::{Path, Token};
use xpute_kit::wire::xtp::TreeReader;

/// A frame's room in the test's front.
const FRAME: u32 = 256;
/// Where the test keeps what an import names: past both frames, which a
/// packet laid in the send frame would write over.
pub(crate) const SCRATCH: usize = 2 * FRAME as usize;
const PROGRAM: u32 = frame::cmd(frame::PROGRAM_MAJOR, 1);

/// What the core sent: its frame, its packet and the memory it named.
pub(crate) struct Sent {
    pub frame: Frame,
    pub pkt: Vec<u8>,
    pub named: Vec<u8>,
}

thread_local! {
    static MEM: Cell<Option<Memory>> = const { Cell::new(None) };
    static WAITING: RefCell<VecDeque<(Frame, Vec<u8>)>> = const { RefCell::new(VecDeque::new()) };
    static SENT: RefCell<Vec<Sent>> = const { RefCell::new(Vec::new()) };
}

fn mem() -> Memory {
    MEM.with(Cell::get).expect("a test's door")
}

fn bytes(at: u32, len: usize) -> Vec<u8> {
    // SAFETY: the test's memory, or the memory a frame names, which lives
    // through the call.
    unsafe { core::slice::from_raw_parts(mem().at::<u8>(at as usize), len) }.to_vec()
}

/// The runtime's end of the test: what `submit` queued is received, and what
/// the core sends is kept for the test to read (`sent`). A call is refused.
fn receive(at: u32, _cap: u32) -> i32 {
    let Some((mut f, pkt)) = WAITING.with(|w| w.borrow_mut().pop_front()) else {
        return 0;
    };
    f.pkt_len = pkt.len() as u32;
    f.lay(mem(), at);
    // SAFETY: the receive frame's room, which the door gave.
    let room = unsafe {
        core::slice::from_raw_parts_mut(mem().at::<u8>((at + WORD_BYTES) as usize), pkt.len())
    };
    room.copy_from_slice(&pkt);
    1
}

fn send(at: u32, named_at: u32, named_len: u32) {
    let f = Frame::at(mem(), at);
    let pkt = f.pkt(mem(), at, FRAME).expect("a frame laid whole");
    let sent = Sent {
        frame: f,
        pkt: pkt.map(<[u8]>::to_vec).unwrap_or_default(),
        named: bytes(named_at, named_len as usize),
    };
    SENT.with(|s| s.borrow_mut().push(sent));
}

/// A door over a front of its own, with the test's runtime handed to it.
pub(crate) fn door() -> (Door, Box<[u8]>) {
    let mut buf = vec![0u8; SCRATCH + 64].into_boxed_slice();
    // SAFETY: the buffer outlives the door: both are returned together, and
    // the caller holds them for the test.
    let mem = unsafe { Memory::new(buf.as_mut_ptr().expose_provenance(), buf.len()) };
    MEM.with(|m| m.set(Some(mem)));
    WAITING.with(|w| w.borrow_mut().clear());
    SENT.with(|s| s.borrow_mut().clear());
    host::set(Host {
        receive,
        send,
        call: |_| Errno::ENOSYS as i32,
    });
    (Door::new(mem, 0, FRAME, FRAME), buf)
}

/// Queues a frame for the core to receive.
pub(crate) fn submit(tag: u32, cmd: u32, flags: u32, result: i32, pkt: &[u8]) {
    let f = Frame {
        tag,
        cmd,
        flags,
        result,
        pkt_len: 0,
    };
    WAITING.with(|w| w.borrow_mut().push_back((f, pkt.to_vec())));
}

/// What the core has sent since the door was made, taken.
pub(crate) fn sent() -> Vec<Sent> {
    SENT.with(|s| core::mem::take(&mut *s.borrow_mut()))
}

/// A packet of one u32, `val`: NOP's, or an open's reply.
fn one(val: u32) -> Vec<u8> {
    let mut buf = vec![0u8; FRAME as usize];
    let mut w = PacketWriter::new(&mut buf, 1);
    w.u32(Some(val));
    let len = w.fit_len().expect("a packet that fits a frame") as usize;
    buf.truncate(len);
    buf
}

#[test]
fn door_answers_nop() {
    let (d, _buf) = door();
    submit(4, Command::NOP as u32, frame::ACKREQ, 0, &one(7));
    d.receive(|_| unreachable!("the program is handed no NOP"));
    let f = sent().remove(0).frame;
    assert_eq!(
        (f.tag, f.cmd, f.flags & frame::RES, f.result),
        (4, Command::NOP as u32, frame::RES, 7)
    );
}

#[test]
fn every_waiting_frame_applied_in_one_turn() {
    let (d, _buf) = door();
    for tag in 1..=20 {
        submit(tag, PROGRAM, frame::ACKREQ, 0, &[]);
    }
    d.receive(|_| Ok(Some(0.into())));
    let tags: Vec<u32> = sent().iter().map(|s| s.frame.tag).collect();
    assert_eq!(tags, (1..=20).collect::<Vec<_>>(), "each once, in order");
}

#[test]
fn open_completion_handed_over() {
    let (d, _buf) = door();
    submit(3, fs::Command::FS_OPEN as u32, frame::RES, 9, &one(404));
    let mut got = None;
    d.receive(|event| match event {
        Event::Completion(c) => {
            got = fs::completion(&c).map(|c| match c {
                fs::Completion::FsOpen { fd, res, reply } => (fd, res, reply.map(|r| r.detail)),
                fs::Completion::FsClose { .. } => unreachable!("an open's"),
            });
            Ok(Some(0.into()))
        }
        Event::Life(_)
        | Event::Turn(_)
        | Event::Frame(_)
        | Event::Changed(_)
        | Event::Program(..) => {
            unreachable!("an open's completion is not the program's frame")
        }
    });
    assert_eq!(got, Some((3, Ok(9), Some(404))));
    assert!(sent().is_empty(), "a completion is answered by nothing");
}

#[test]
fn program_cmd_handed_over() {
    let (d, _buf) = door();
    let tag = 6;
    submit(tag, PROGRAM, frame::ACKREQ, 0, &[]);
    let mut got = None;
    d.receive(|event| match event {
        Event::Program(cmd, _) => {
            got = Some(cmd);
            Ok(Some(1.into()))
        }
        Event::Life(_)
        | Event::Turn(_)
        | Event::Frame(_)
        | Event::Completion(_)
        | Event::Changed(_) => {
            unreachable!("a command past xpute's majors is the program's")
        }
    });
    assert_eq!(got, Some(PROGRAM));
    let f = sent().remove(0).frame;
    assert_eq!(
        (f.tag, f.result),
        (tag, 1),
        "the reply goes back to its sender"
    );
}

#[test]
fn xpute_number_unknown_refused() {
    let (d, _buf) = door();
    submit(5, 0x01f1, 0, 0, &[]);
    d.receive(|_| unreachable!("a number under xpute's majors is none of the program's"));
    let f = sent().remove(0).frame;
    assert_eq!((f.tag, f.result), (5, Errno::ENOSYS as i32));
}

struct Ping;

impl Import for Ping {
    const NUMBER: u32 = 0x1201;
    const FIELDS: u32 = 0;
    const ACKREQ: bool = false;
    fn write(&self, _: &mut PacketWriter) {}
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// An import that names `len` bytes of the test's memory at `at`.
struct Shown {
    at: u32,
    len: u32,
}

impl Import for Shown {
    const NUMBER: u32 = 0x1202;
    const FIELDS: u32 = 0;
    const ACKREQ: bool = false;
    fn write(&self, _: &mut PacketWriter) {}
    fn named(&self) -> (u32, u32) {
        (self.at, self.len)
    }
}

#[test]
fn call_sends_nothing() {
    let (d, _buf) = door();
    d.send(&Ping);
    assert_eq!(
        d.call(&Ping),
        Err(Errno::ENOSYS),
        "the test's runtime refuses it"
    );
    assert_eq!(sent().len(), 1, "a call is answered, not sent");
}

/// The memory an import names goes with its frame, as it was when sent.
#[test]
fn named_memory_taken_with_the_frame() {
    let (d, mut buf) = door();
    let at = SCRATCH;
    buf[at..at + 3].copy_from_slice(&[7, 8, 9]);
    d.send(&Shown {
        at: at as u32,
        len: 3,
    });
    buf[at] = 0;
    assert_eq!(sent().remove(0).named, vec![7, 8, 9]);
}

/// The memory a reply names goes with its frame, as it was when sent.
#[test]
fn reply_named_memory_taken_with_the_frame() {
    let (d, mut buf) = door();
    buf[SCRATCH..SCRATCH + 3].copy_from_slice(&[7, 8, 9]);
    // SAFETY: the test's memory, which outlives the receive.
    let named: &'static [u8] = unsafe { core::slice::from_raw_parts(buf.as_ptr().add(SCRATCH), 3) };
    submit(5, PROGRAM, frame::ACKREQ, 0, &[]);
    d.receive(|_| Ok(Some(Reply { val: 2, named })));
    let s = sent().remove(0);
    assert_eq!(
        (s.frame.tag, s.frame.flags & frame::RES, s.frame.result),
        (5, frame::RES, 2)
    );
    assert_eq!(s.named, vec![7, 8, 9]);
}

/// Memory a reply names outside the core's memory, as a native build's
/// static is, is the core's bug.
#[test]
#[should_panic(expected = "EFAULT")]
fn reply_naming_memory_outside_the_core_stops_it() {
    static OUTSIDE: [u8; 1] = [0];
    let (d, _buf) = door();
    submit(5, PROGRAM, frame::ACKREQ, 0, &[]);
    d.receive(|_| {
        Ok(Some(Reply {
            val: 0,
            named: &OUTSIDE,
        }))
    });
}

/// A stop report with no file path takes `ABORT_PKT_BYTES`, as the writer
/// lays it.
#[test]
fn abort_pkt_bytes_of_the_format() {
    let report = SysAbort {
        errnum: -1,
        line: u32::MAX,
        column: u32::MAX,
        a_low: u32::MAX,
        a_high: u32::MAX,
        b_low: u32::MAX,
        b_high: u32::MAX,
        file: &"",
    };
    let mut buf = [0u8; 256];
    let mut w = PacketWriter::new(&mut buf, SysAbort::FIELDS);
    report.write(&mut w);
    assert_eq!(w.finish().unwrap(), ABORT_PKT_BYTES);
}

#[test]
fn abort_sends_the_report_after_what_was_sent() {
    let (d, _buf) = door();
    d.send(&Ping);
    d.send(&Ping);
    d.abort(Errno::EINVAL, core::panic::Location::caller(), 1, 2);
    let sent = sent();
    assert_eq!(sent.len(), 3, "nothing sent is taken back");
    assert_eq!(sent[2].frame.tag, 0);
    assert!(
        !sent[2].pkt.is_empty(),
        "the report carries where it stopped"
    );
    assert_eq!(
        sent[2].frame.cmd,
        crate::abi::sys::Command::SYS_ABORT as u32
    );
}

/// A path's token is a string or a number that is an i32; any other node, a
/// fraction, a bool, an integer past an i32 or a branch, is refused as it is
/// read.
#[test]
fn path_takes_i32_and_str_tokens() {
    let mut buf = [0u8; 128];
    let mut w = PacketWriter::new(&mut buf, 1);
    w.branch(3, |w| {
        w.i32(Some(i32::MIN)).str(Some("0123")).i32(Some(i32::MAX));
    });
    let n = w.finish().unwrap() as usize;
    let f = TreeReader::new(&buf[..n]).unwrap().read_branch().unwrap();
    let path = Path::read(&buf[..n], f.at(0).unwrap()).unwrap();
    assert_eq!(
        path.iter().collect::<Vec<_>>(),
        [
            Token::Int(i32::MIN),
            Token::Str("0123"),
            Token::Int(i32::MAX)
        ]
    );
    for bad in [
        |w: &mut PacketWriter| {
            w.f64(Some(0.5));
        },
        |w: &mut PacketWriter| {
            w.bool(Some(true));
        },
        |w: &mut PacketWriter| {
            w.u32(Some(u32::MAX));
        },
        |w: &mut PacketWriter| {
            w.branch(0, |_| {});
        },
    ] {
        let mut buf = [0u8; 64];
        let mut w = PacketWriter::new(&mut buf, 1);
        w.branch(1, |w| bad(w));
        let n = w.finish().unwrap() as usize;
        let f = TreeReader::new(&buf[..n]).unwrap().read_branch().unwrap();
        assert!(Path::read(&buf[..n], f.at(0).unwrap()).is_err());
    }
}
