// xpute-runtime/native/serve.rs

//! This is the runtime's own thread on a device: the one writer of the
//! submission ring and the one reader of the completion ring, so that the UI's
//! thread holds no runtime, as a page's main thread holds none. The UI reaches
//! it as a page's UI reaches its worker: what the UI sends is queued here
//! (`post`), and what the UI hears is copied out of the rings and handed over
//! in a batch (`take`), with the memory each frame names, so that the head can
//! pass the frame at once (R3).
//!
//! The file device is served here. A read is copied into the core's memory
//! before the head passes its frame, from the bytes the program handed over
//! when the open completed. What backs a file stays the program's, on the UI's
//! thread (`fs_opened`, `fs_closed`).

use core::mem::take as replace_empty;
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, MutexGuard, Once, OnceLock, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use xpute_core::abi::fs::{Command as FsCommand, FsClose, FsOpenat, FsRead, FsWrite};
use xpute_core::abi::sys::{Command as SysCommand, SysPace};
use xpute_core::ipc::frame::{self, Frame};
use xpute_core::ipc::fs::spec::flag;
use xpute_core::ipc::ring::Ring;
use xpute_core::mem::Memory;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::PacketWriter;

use super::thread::{self, Grant};

/// The command of the frame that says the core has stopped; it is the last the
/// UI hears.
pub const STOPPED: u32 = u32::MAX;
/// The tag of the call `call` waits on; the UI's own ids stay below it.
const CALL_TAG: u32 = u32::MAX;
/// How long `call` waits for this thread to read the reply before it rings
/// again.
const CALL_WAIT: Duration = Duration::from_millis(1);

/// The words before a frame's bytes in a batch: its tag, command, flags, result
/// and the lengths of its packet and of the memory it names.
const DOWN_WORDS: usize = 6;
const DOWN_ALIGN: usize = 8;
/// Room for a packet the runtime writes itself.
const PACKET_BYTES: usize = 256;

/// What a program says of the memory a frame names, where it is and how long.
pub type Named = fn(cmd: u32, pkt: &[u8]) -> Option<(u32, u32)>;

struct Up {
    tag: u32,
    cmd: u32,
    flags: u32,
    result: i32,
    pkt: Vec<u8>,
}

struct File {
    opened: bool,
    bytes: Option<Vec<u8>>,
    /// Where each write's bytes lie in the core's memory, in order; none for a
    /// file opened to be read.
    written: Option<Vec<(u32, u32)>>,
}

struct Shared {
    /// Something waits for this thread: a frame to submit, or a turn's posts.
    dirty: bool,
    up: Vec<Up>,
    /// A frame's grant to put on the ring, then kick its turn.
    frame: Option<Grant>,
    /// A frame's grant is on its way into the ring with `up`; its turn is
    /// kicked once `up` is all in.
    kick_after: Option<Grant>,
    down: Vec<u8>,
    /// The UI has been told of `down` and has not taken it.
    told: bool,
    /// The reply to the call under way, once it is read.
    answer: Option<i32>,
    stop_told: bool,
    /// The machine asks for the file device.
    fs: bool,
    files: HashMap<u32, File>,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
static CV: Condvar = Condvar::new();
static ANSWERED: Condvar = Condvar::new();
static STARTED: Once = Once::new();
static NAMED: OnceLock<Named> = OnceLock::new();

fn fresh() -> Shared {
    Shared {
        dirty: true,
        up: Vec::new(),
        frame: None,
        kick_after: None,
        down: Vec::new(),
        told: false,
        answer: None,
        stop_told: false,
        fs: false,
        files: HashMap::new(),
    }
}

fn lock() -> MutexGuard<'static, Option<Shared>> {
    SHARED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f` on the shared state, once `attach` has made it.
fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
    lock().as_mut().map(f)
}

/// The program says which memory its frames name.
pub fn named_by(named: Named) {
    let _ = NAMED.set(named);
}

/// A turn ended or stalled, or the core stopped: the rings want reading.
pub(super) fn news() {
    if with(|s| s.dirty = true).is_some() {
        CV.notify_all();
    }
}

/// Starts serving the rings at `sq_at` and `cq_at`, the first time, and forgets
/// what the UI that attached before this one left. A core outlives a reload of
/// the UI.
///
/// # Safety
/// `base..base + bytes` is the core's memory, with its rings laid.
pub unsafe fn attach(base: usize, bytes: usize, sq_at: u32, cq_at: u32, fs: bool) {
    *lock() = Some(Shared { fs, ..fresh() });
    STARTED.call_once(|| {
        // SAFETY: as this function's caller says.
        let (mem, sq, cq) = unsafe {
            let mem = Memory::new(base, bytes);
            (
                mem,
                Ring::new(mem, sq_at as usize),
                Ring::new(mem, cq_at as usize),
            )
        };
        std::thread::Builder::new()
            .name("xpute-runtime".into())
            .spawn(move || serve(mem, sq, cq))
            .expect("the system would not start the runtime's thread");
    });
    CV.notify_all();
}

fn serve(mem: Memory, sq: Ring, cq: Ring) {
    loop {
        let (up, frame, kick_after) = {
            let mut g = lock();
            loop {
                if let Some(s) = g.as_mut() {
                    if replace_empty(&mut s.dirty) {
                        break (
                            replace_empty(&mut s.up),
                            s.frame.take(),
                            s.kick_after.take(),
                        );
                    }
                }
                g = CV.wait(g).unwrap_or_else(PoisonError::into_inner);
            }
        };
        // A grant goes on only while turns run, so none waits in the ring for
        // a turn rung while paused.
        let frame = frame.filter(|_| thread::running());
        // What the UI sent asks for a turn; a grant brings its own (`kick`).
        let pushed = submit(&sq, up);
        if let Some(grant) = frame {
            // The grant follows whatever the ring had no room for yet, in order.
            let waiting = with(|s| !s.up.is_empty()) == Some(true);
            if waiting {
                with(|s| s.up.extend(granted(grant)));
            } else {
                submit(&sq, granted(grant).into());
            }
        }
        if let Some(grant) = frame.or(kick_after) {
            if with(|s| s.up.is_empty()) == Some(true) {
                thread::kick(grant.deadline_at);
            } else {
                with(|s| s.kick_after = Some(grant));
            }
        }
        // Only a turn makes room in a full ring, so one is begun for what waits.
        if with(|s| !s.up.is_empty()) == Some(true) {
            thread::drain();
        }
        // Whether the core has stopped is read before the ring, so that its
        // report, posted before it stopped, is read first.
        let stopped = thread::terminated();
        let read = read_dry(mem, &cq);
        if pushed || read {
            thread::wake();
        }
        let tell = with(|s| {
            if stopped && !replace_empty(&mut s.stop_told) {
                s.stop_told = true;
                down(s, 0, STOPPED, 0, 0, &[], &[]);
            }
            !s.down.is_empty() && !core::mem::replace(&mut s.told, true)
        });
        if tell == Some(true) {
            thread::notify();
        }
    }
}

/// A frame's grant as the frames that carry it: what frame pacing says of the
/// turn (SYS_FRAME), then the turn with the wall clock (SYS_TURN).
fn granted(grant: Grant) -> [Up; 2] {
    let frame = packet(3, |w| {
        w.f64(Some(grant.refresh_ms))
            .f64(Some(grant.paced_ms))
            .bool(Some(grant.interactive));
    });
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    let turn = packet(1, |w| {
        w.f64(Some(unix_ms));
    });
    let up = |cmd: SysCommand, pkt: Vec<u8>| Up {
        tag: 0,
        cmd: cmd as u32,
        flags: 0,
        result: 0,
        pkt,
    };
    [
        up(SysCommand::SYS_FRAME, frame),
        up(SysCommand::SYS_TURN, turn),
    ]
}

/// The bytes of a packet of `cnt` children that `write` writes. The runtime's
/// own packets are a few numbers, far under `PACKET_BYTES`.
fn packet(cnt: u32, write: impl FnOnce(&mut PacketWriter)) -> Vec<u8> {
    let mut buf = [0u8; PACKET_BYTES];
    let mut w = PacketWriter::new(&mut buf, cnt);
    write(&mut w);
    let len = w
        .finish()
        .unwrap_or_else(|e| xpute_kit::bug!(EOVERFLOW, e.errnum as u32));
    buf[..len as usize].to_vec()
}

/// A display frame's grant, from `vsync`. Before a runtime has attached it is
/// dropped, and the next frame grants again.
pub(super) fn frame(grant: Grant) {
    with(|s| {
        s.frame = Some(grant);
        s.dirty = true;
    });
    CV.notify_all();
}

/// Writes what waits into the submission ring, and keeps what the ring has no
/// room for until a turn has made some.
fn submit(sq: &Ring, up: Vec<Up>) -> bool {
    let mut pushed = false;
    let mut rest = up.into_iter();
    while let Some(u) = rest.next() {
        let pkt = (!u.pkt.is_empty()).then_some(u.pkt.as_slice());
        if sq.push(u.tag, u.cmd, u.flags, u.result, pkt) == Errno::EAGAIN {
            let left: Vec<Up> = core::iter::once(u).chain(rest).collect();
            with(|s| s.up.splice(0..0, left).for_each(drop));
            break;
        }
        pushed = true;
    }
    pushed
}

fn down(s: &mut Shared, tag: u32, cmd: u32, flags: u32, result: i32, pkt: &[u8], named: &[u8]) {
    let head = [
        tag,
        cmd,
        flags,
        result as u32,
        pkt.len() as u32,
        named.len() as u32,
    ];
    debug_assert_eq!(head.len(), DOWN_WORDS);
    for w in head {
        s.down.extend_from_slice(&w.to_le_bytes());
    }
    for bytes in [pkt, named] {
        s.down.extend_from_slice(bytes);
        s.down.resize(s.down.len().next_multiple_of(DOWN_ALIGN), 0);
    }
}

fn bytes_of(mem: Memory, at: u32, len: u32) -> &'static [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the bytes a frame names, the runtime's until its head passes
    // that frame (R3); the memory lives as long as the process.
    unsafe { core::slice::from_raw_parts(mem.at::<u8>(at as usize), len as usize) }
}

/// Reads the completion ring dry, and says whether it took anything.
fn read_dry(mem: Memory, cq: &Ring) -> bool {
    let mut read = false;
    while let Some(f) = cq.peek() {
        let pkt = cq.pkt(&f).unwrap_or_default();
        with(|s| take_frame(s, mem, &f, pkt));
        cq.advance();
        read = true;
    }
    if read {
        ANSWERED.notify_all();
    }
    read
}

fn take_frame(s: &mut Shared, mem: Memory, f: &Frame, pkt: Option<&[u8]>) {
    if f.tag == CALL_TAG && f.flags & frame::RES != 0 {
        s.answer = Some(f.result);
        return;
    }
    let body = pkt.unwrap_or(&[]);
    if f.cmd == SysCommand::SYS_PACE as u32 {
        if let Ok(c) = SysPace::read(body) {
            thread::ask(c.interval_ms);
        }
        return;
    }
    if f.flags & frame::RES == 0 && take_file(s, mem, f, body) {
        return;
    }
    let named = NAMED
        .get()
        .and_then(|of| of(f.cmd, body))
        .map_or(&[][..], |(at, len)| bytes_of(mem, at, len));
    down(s, f.tag, f.cmd, f.flags, f.result, body, named);
}

/// Serves a frame of the file device, and says whether nothing more is to be
/// done with it.
fn take_file(s: &mut Shared, mem: Memory, f: &Frame, pkt: &[u8]) -> bool {
    let Some(cmd) = FsCommand::of(f.cmd) else {
        return false;
    };
    if !s.fs {
        // A machine with no file device: an open or a close, which asks for a
        // reply, is answered ENODEV, and a read or a write gets nothing back.
        if matches!(cmd, FsCommand::FS_OPENAT | FsCommand::FS_CLOSE) {
            complete(s, f.tag, f.cmd, Errno::ENODEV as i32, Vec::new());
        }
        return true;
    }
    match Some(cmd) {
        Some(FsCommand::FS_OPENAT) => {
            let Ok((fd, _root, flags, path)) = FsOpenat::parts(pkt) else {
                complete_open(s, f.tag, Errno::EBADMSG as i32, 0);
                return true;
            };
            if !walks_down(path) {
                complete_open(s, fd, Errno::EACCES as i32, 0);
                return true;
            }
            let write = flags == flag::O_WRONLY;
            s.files.insert(
                fd,
                File {
                    opened: write,
                    bytes: None,
                    written: write.then(Vec::new),
                },
            );
            if write {
                complete_open(s, fd, 0, 0);
            }
            false
        }
        Some(FsCommand::FS_READ) => {
            if let Ok(c) = FsRead::read(pkt) {
                if let Some(bytes) = s.files.get(&c.fd).and_then(|file| file.bytes.as_ref()) {
                    let len = (c.len as usize).min(bytes.len());
                    // SAFETY: the bytes the frame names, the runtime's until its
                    // head passes the frame (R3).
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            mem.at::<u8>(c.at as usize),
                            len,
                        );
                    }
                }
            }
            true
        }
        Some(FsCommand::FS_WRITE) => {
            if let Ok(c) = FsWrite::read(pkt) {
                if let Some(written) = s.files.get_mut(&c.fd).and_then(|f| f.written.as_mut()) {
                    written.push((c.at, c.len));
                }
            }
            true
        }
        Some(FsCommand::FS_CLOSE) => {
            if let Ok(c) = FsClose::read(pkt) {
                close(s, mem, c.fd);
            }
            true
        }
        _ => false,
    }
}

/// Whether no name in `path` is empty, `.` or `..`, so that the path cannot
/// leave its root.
fn walks_down(path: &str) -> bool {
    path.split('/')
        .all(|n| !n.is_empty() && n != "." && n != "..")
}

fn close(s: &mut Shared, mem: Memory, fd: u32) {
    let close = FsCommand::FS_CLOSE as u32;
    let Some(file) = s.files.remove(&fd) else {
        complete(s, fd, close, Errno::EBADF as i32, Vec::new());
        return;
    };
    if !file.opened {
        complete_open(s, fd, Errno::ECANCELED as i32, 0);
        down(s, fd, close, 0, 0, &[], &[]);
    }
    let Some(written) = file.written else {
        // A read file is let go by the program as well.
        if file.opened {
            down(s, fd, close, 0, 0, &[], &[]);
        }
        complete(s, fd, close, 0, Vec::new());
        return;
    };
    let mut whole = Vec::with_capacity(written.iter().map(|&(_, n)| n as usize).sum());
    for (at, len) in written {
        whole.extend_from_slice(bytes_of(mem, at, len));
    }
    // A result of 1 asks the program to commit what the frame names.
    down(s, fd, close, 0, 1, &[], &whole);
}

fn complete(s: &mut Shared, tag: u32, cmd: u32, result: i32, pkt: Vec<u8>) {
    s.up.push(Up {
        tag,
        cmd,
        flags: frame::RES,
        result,
        pkt,
    });
    s.dirty = true;
}

fn complete_open(s: &mut Shared, fd: u32, result: i32, detail: u32) {
    let pkt = packet(1, |w| {
        w.u32(Some(detail));
    });
    complete(s, fd, FsCommand::FS_OPENAT as u32, result, pkt);
}

/// The program's bytes for the open of `fd`, or its refusal: a negative `result`
/// and the number the store refused with.
pub fn fs_opened(fd: u32, result: i32, detail: u32, bytes: Option<Vec<u8>>) {
    with(|s| {
        let Some(file) = s.files.get_mut(&fd) else {
            return;
        };
        file.opened = true;
        file.bytes = bytes;
        if result < 0 {
            s.files.remove(&fd);
        }
        complete_open(s, fd, result, detail);
    });
    CV.notify_all();
}

/// The program's answer to the commit of `fd`.
pub fn fs_closed(fd: u32, result: i32) {
    with(|s| complete(s, fd, FsCommand::FS_CLOSE as u32, result, Vec::new()));
    CV.notify_all();
}

/// Queues a frame for the submission ring.
pub fn post(tag: u32, cmd: u32, flags: u32, result: i32, pkt: &[u8]) {
    with(|s| {
        s.up.push(Up {
            tag,
            cmd,
            flags,
            result,
            pkt: pkt.to_vec(),
        });
        s.dirty = true;
    });
    CV.notify_all();
}

/// Hands over what the UI has not heard: frames of `DOWN_WORDS` little-endian
/// words, then the packet and the memory named, each padded to `DOWN_ALIGN`.
pub fn take() -> Vec<u8> {
    with(|s| {
        s.told = false;
        replace_empty(&mut s.down)
    })
    .unwrap_or_default()
}

/// Calls the core: submits `cmd` with ACKREQ and rings until its reply has
/// been read, answering the turns' calls meanwhile. It answers the reply's
/// result, or ENOTRECOVERABLE once the core has stopped. One call is under way
/// at a time, on the UI's thread.
pub fn call(cmd: u32, pkt: &[u8]) -> i32 {
    if with(|s| s.answer = None).is_none() {
        return Errno::ENOTRECOVERABLE as i32;
    }
    post(CALL_TAG, cmd, frame::ACKREQ, 0, pkt);
    loop {
        if thread::ring(0.0).is_none() {
            return Errno::ENOTRECOVERABLE as i32;
        }
        let g = lock();
        let (g, _) = ANSWERED
            .wait_timeout_while(g, CALL_WAIT, |g| {
                g.as_ref().is_some_and(|s| s.answer.is_none())
            })
            .unwrap_or_else(PoisonError::into_inner);
        match g.as_ref().map(|s| s.answer) {
            Some(Some(res)) => return res,
            Some(None) => {}
            None => return Errno::ENOTRECOVERABLE as i32,
        }
    }
}
