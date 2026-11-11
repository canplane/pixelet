// xpute-core/io/file.test.rs

use super::*;
use crate::abi::fs::{Command, FsOpen};
use crate::abi::handle::handle_slot;
use crate::io::door::test::{door, sent, SCRATCH};
use crate::io::frame;
use crate::io::fs::spec::flag;
use crate::io::fs::Token;

type Files = FileTable<&'static str, &'static str, Vec<u8>, 4>;

const PATH: [Token; 2] = [Token::U32(7), Token::Str("a")];

/// A runtime's result code as a completion carries it.
fn res(code: i32) -> Result<u32, Errno> {
    crate::host::answer_of(code)
}

/// The test's door, made the core's, under the lock every test that touches a
/// `Global` takes.
fn global() -> (std::sync::MutexGuard<'static, ()>, Box<[u8]>) {
    let lock = crate::global::serial();
    let (d, buf) = door();
    crate::io::door::set(d);
    (lock, buf)
}

fn cmds() -> Vec<u32> {
    sent().iter().map(|s| s.frame.cmd).collect()
}

fn fs(c: Command) -> u32 {
    c as u32
}

/// A descriptor is let go as its close is sent, so its slot is taken again
/// at once, under another tag; the old open's completion reaches no one, and
/// the old descriptor is not one to close again.
#[test]
fn let_go_at_close() {
    let ((_lock, _buf), mut files) = (global(), Files::new());
    let a = files.open(&PATH, "first");
    files.close(a).unwrap();
    let b = files.open(&PATH, "second");
    assert_eq!(handle_slot(a), handle_slot(b), "the slot is free again");
    assert_ne!(a, b, "under another tag");
    assert_eq!(files.complete_open(a, res(-125), 404), None);
    assert_eq!(files.close(a), Err(Errno::EBADF));
    assert_eq!(files.find(|o| *o == "second"), Some(b));
    assert!(files.complete_close(a, res(0)).is_none());
}

/// An open and a close are tagged with the descriptor and ask for
/// completion; the path goes as its tokens, each its own type.
#[test]
fn open_and_close_ask_for_completion() {
    let ((_lock, _buf), mut files) = (global(), Files::new());
    let fd = files.open(&PATH, "first");
    files.close(fd).unwrap();
    let sent = sent();
    let (path, flags) = FsOpen::parts(&sent[0].pkt).unwrap();
    assert_eq!(flags, flag::O_RDONLY);
    assert_eq!(path.iter().collect::<Vec<_>>(), PATH);
    for (s, cmd) in sent.iter().zip([Command::FS_OPEN, Command::FS_CLOSE]) {
        let f = s.frame;
        assert_eq!(
            (f.tag, f.cmd, f.flags & frame::ACKREQ),
            (fd, cmd as u32, frame::ACKREQ)
        );
    }
}

#[test]
fn failed_open_let_go() {
    let ((_lock, _buf), mut files) = (global(), Files::new());
    let failed = files.open(&PATH, "first");
    assert_eq!(
        files.complete_open(failed, res(-2), 404),
        Some(("first", OpenResult::Absent))
    );
    assert_eq!(
        files.close(failed),
        Err(Errno::EBADF),
        "a failed open leaves no descriptor"
    );
    let open = files.open(&PATH, "second");
    assert_eq!(
        files.complete_open(open, res(5), 404),
        Some(("second", OpenResult::Bytes(5)))
    );
    assert_eq!(files.find(|o| *o == "second"), Some(open), "it stays open");
}

#[test]
fn completion_read_once() {
    assert_eq!(OpenResult::of(res(5), 404), OpenResult::Bytes(5));
    assert_eq!(OpenResult::of(res(-2), 404), OpenResult::Absent);
    assert_eq!(
        OpenResult::of(res(-5), 404),
        OpenResult::Failed {
            errnum: Errno::EIO,
            detail: 404
        }
    );
    assert_eq!(
        OpenResult::of(res(-4095), 404),
        OpenResult::Failed {
            errnum: Errno::EPROTO,
            detail: 404
        },
        "a code no errno names breaks the protocol"
    );
}

/// A read's request and its buffer are held until its close is completed, and
/// handed back then with the close's result, though its descriptor was let go
/// as the close was sent. Every frame of the read is tagged with the
/// descriptor. The heap lies outside the test's memory, so the buffer is
/// empty.
#[test]
fn read_whole_handed_at_close() {
    let ((_lock, _buf), mut files) = (global(), Files::new());
    let fd = files.open(&PATH, "reader");
    files.read_whole(fd, Vec::new(), "read");
    assert_eq!(
        cmds(),
        [
            fs(Command::FS_OPEN),
            fs(Command::FS_READ),
            fs(Command::FS_CLOSE)
        ]
    );
    assert!(sent().iter().all(|s| s.frame.tag == fd));
    assert_eq!(files.find(|_| true), None, "let go as its close is sent");
    assert_eq!(
        files.complete_close(fd, res(0)),
        Some(("read", Some(Vec::new()), Ok(())))
    );
    let fd = files.open(&PATH, "reader");
    files.read_whole(fd, Vec::new(), "read");
    assert_eq!(
        files.complete_close(fd, res(-9)),
        Some(("read", Some(Vec::new()), Err(Errno::EBADF)))
    );
}

/// A written file is opened to write, written and closed in one turn, each
/// frame tagged with its descriptor and the bytes going with their frame; it
/// takes no slot, and its request hears the close's result once.
#[test]
fn write_whole_request_hears_its_close() {
    let ((_lock, mut buf), mut files) = (global(), Files::new());
    buf[SCRATCH..SCRATCH + 2].copy_from_slice(&[1, 2]);
    let fd = files.write_whole(&PATH, &buf[SCRATCH..SCRATCH + 2], "writer");
    let sent = sent();
    let got: Vec<u32> = sent.iter().map(|s| s.frame.cmd).collect();
    assert_eq!(
        got,
        [
            fs(Command::FS_OPEN),
            fs(Command::FS_WRITE),
            fs(Command::FS_CLOSE)
        ]
    );
    assert!(sent.iter().all(|s| s.frame.tag == fd));
    assert_eq!(handle_slot(fd), 0, "it takes no slot");
    assert_eq!(FsOpen::parts(&sent[0].pkt).unwrap().1, flag::O_WRONLY);
    assert_eq!(sent[1].named, vec![1, 2]);
    assert_eq!(
        files.complete_open(fd, res(0), 404),
        None,
        "it holds no slot"
    );
    let next = files.write_whole(&PATH, &buf[SCRATCH..SCRATCH], "next");
    assert_ne!(next, fd, "under another tag");
    let refused = res(-30);
    assert_eq!(
        files.complete_close(fd, refused),
        Some(("writer", None, Err(Errno::EROFS)))
    );
    assert_eq!(files.complete_close(fd, refused), None, "heard once");
}
