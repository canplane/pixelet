// xpute-runtime/native/serve.rs

//! This is the runtime's own thread on a device, which serves what the core
//! sends, so that the UI's thread holds no runtime, as a page's main thread
//! holds none. The core's thread receives and sends through `receive` and
//! `send` (xpute-core host.rs), which only copy a frame into or out of the
//! queue. The UI reaches the core as a page's UI reaches its worker: what the
//! UI sends is queued for the core (`post`), and what the UI hears is handed
//! over in a batch (`take`), with the memory each frame names.
//!
//! The file device is served here. A read is copied into the core's memory
//! before its descriptor's close is completed, from the bytes the program
//! handed over when the open completed. An open to write is completed here at
//! once, and what is written is held here until the file's close. What backs
//! a file stays the program's, on the UI's thread: it opens a file to read
//! (`fs_complete_open`), and, at a written file's close, it is asked to commit the
//! whole file, and answers with the close's result (`post`). A read of a file
//! opened to write, or a write of one opened to read, leaves the file alone,
//! and its close completes with EBADF here. What the UI is handed is a batch
//! (spec/io/batch.json).
//!
//! Two locks guard the state. `QUEUE` holds the frames between the core and
//! this thread, and the core's thread takes it only to copy a frame in or out,
//! with nothing in between that can panic or wait, so a stop reported from
//! the core's panic hook never waits on itself. `SHARED` holds the rest, and
//! the core's thread never takes it. Where both are held, `SHARED` is taken
//! first.

use core::mem::take as replace_empty;
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, MutexGuard, Once, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use xpute_core::abi::fs::{Command as FsCommand, FsOpen, FsRead};
use xpute_core::abi::sys::{Command as SysCommand, SysPace};
use xpute_core::io::frame::{self, Frame, WORD_BYTES};
use xpute_core::io::fs::spec::flag;
use xpute_core::io::fs::Path;
use xpute_core::mem::Memory;
use xpute_kit::status::errno::Errno;
use xpute_kit::wire::xtp::PacketWriter;

use super::batch::{self, ALIGN, COMMIT, HEADER_WORDS, STOPPED};
use super::thread::{self, Grant};

/// Room for a packet the runtime writes itself.
const PKT_BYTES: usize = 256;

/// A frame for the core.
struct Up {
    tag: u32,
    cmd: u32,
    flags: u32,
    result: i32,
    pkt: Vec<u8>,
}

/// A frame the core sent, with the memory it named.
struct Down {
    tag: u32,
    cmd: u32,
    flags: u32,
    result: i32,
    pkt: Vec<u8>,
    named: Vec<u8>,
}

struct Queue {
    /// Something waits for this thread: a frame the core sent, a frame for
    /// the core, a display frame's grant, or a turn's end.
    dirty: bool,
    up: VecDeque<Up>,
    down: Vec<Down>,
    /// A display frame's grant, to queue for the core, then kick its turn.
    frame: Option<Grant>,
}

/// A file the core holds open, by how it was opened. Its `err` is the errno
/// its close completes with, or 0: EFAULT after a read outside the core's
/// memory, and EBADF after a read or a write against how it was opened.
enum File {
    /// Opened to read: its bytes, once the program's open completes.
    Read { bytes: Option<Vec<u8>>, err: i32 },
    /// Opened to write: a packet of its path alone, what was written to it,
    /// and whether the program has been asked to commit it, which completes
    /// its close.
    Write {
        path: Vec<u8>,
        bytes: Vec<u8>,
        closing: bool,
        err: i32,
    },
}

impl File {
    /// Marks the file's close to complete with `errnum`, unless an errno
    /// already waits.
    fn fail(&mut self, errnum: Errno) {
        let (File::Read { err, .. } | File::Write { err, .. }) = self;
        if *err == 0 {
            *err = errnum as i32;
        }
    }
}

struct Shared {
    down: Vec<u8>,
    /// The UI has been told of `down` and has not taken it.
    told: bool,
    stop_told: bool,
    /// The manifest asks for the file device.
    fs: bool,
    files: HashMap<u32, File>,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
static QUEUE: Mutex<Queue> = Mutex::new(Queue {
    dirty: false,
    up: VecDeque::new(),
    down: Vec::new(),
    frame: None,
});
/// This thread waits on this, under `QUEUE`, for something to do.
static CV: Condvar = Condvar::new();
static STARTED: Once = Once::new();

fn fresh() -> Shared {
    Shared {
        down: Vec::new(),
        told: false,
        stop_told: false,
        fs: false,
        files: HashMap::new(),
    }
}

fn lock() -> MutexGuard<'static, Option<Shared>> {
    SHARED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn queue() -> MutexGuard<'static, Queue> {
    QUEUE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Marks something for this thread to do, and wakes it.
fn mark(f: impl FnOnce(&mut Queue)) {
    let mut q = queue();
    f(&mut q);
    q.dirty = true;
    drop(q);
    CV.notify_all();
}

/// Runs `f` on the shared state, once `attach` has made it.
fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
    lock().as_mut().map(f)
}

/// A turn ended, or the core stopped: what the core sent wants serving.
pub(super) fn news() {
    mark(|_| {});
}

/// Starts serving the core, the first time, and forgets what the UI that
/// attached before this one left. A core outlives a reload of the UI, and so
/// do the frames between it and this thread: what the core sent is still
/// served, and what the last UI had not taken goes to this one, but for the
/// file device's frames, whose files the new program never opened. Each file
/// the last program left is cancelled (`cancel`), and the core's owners send
/// again.
///
/// # Safety
/// `base..base + bytes` is the core's memory, with its front laid.
pub unsafe fn attach(base: usize, bytes: usize, fs: bool) {
    let mut shared = lock();
    let (down, files) = shared
        .take()
        .map(|s| (without_fs(&s.down), cancel(s.files)))
        .unwrap_or_default();
    *shared = Some(Shared {
        fs,
        down,
        files,
        ..fresh()
    });
    drop(shared);
    STARTED.call_once(|| {
        // SAFETY: as this function's caller says.
        let mem = unsafe { Memory::new(base, bytes) };
        std::thread::Builder::new()
            .name("xpute-runtime".into())
            .spawn(move || serve(mem))
            .expect("the system would not start the runtime's thread");
    });
    news();
}

/// Cancels what the last program left of `files`, and answers what goes on
/// without it. An open it had not completed, and a written file it had been
/// asked to commit, are completed with ECANCELED. A file read and a written
/// file not yet closed are held here, and go on as they are.
fn cancel(files: HashMap<u32, File>) -> HashMap<u32, File> {
    let canceled = Errno::ECANCELED as i32;
    files
        .into_iter()
        .filter_map(|(fd, file)| match file {
            File::Read { bytes: None, .. } => {
                complete_open(fd, canceled, 0);
                None
            }
            File::Write { closing: true, .. } => {
                complete(fd, FsCommand::FS_CLOSE as u32, canceled, Vec::new());
                None
            }
            file => Some((fd, file)),
        })
        .collect()
}

/// The frames of `down` but the file device's, its commits among them.
fn without_fs(down: &[u8]) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut at = 0;
    while at < down.len() {
        let word = |k: u32| {
            let w = &down[at + 4 * k as usize..at + 4 * k as usize + 4];
            u32::from_le_bytes([w[0], w[1], w[2], w[3]]) as usize
        };
        let align = ALIGN as usize;
        let end = at + 4 * HEADER_WORDS as usize;
        let end = (end + word(batch::PKT)).next_multiple_of(align);
        let end = (end + word(batch::NAMED)).next_multiple_of(align);
        let cmd = word(batch::CMD) as u32;
        if FsCommand::of(cmd).is_none() && cmd != COMMIT {
            kept.extend_from_slice(&down[at..end]);
        }
        at = end;
    }
    kept
}

/// The core's `receive` (xpute-core host.rs), on the core's thread: writes the
/// next frame for the core into the `cap` bytes at `at`. Room that is not the
/// core's (`in_core`), or at offset 0, is the core's bug.
pub(super) fn receive(at: u32, cap: u32) -> i32 {
    let mem = xpute_core::mem::core();
    xpute_kit::ensure!(at != 0 && in_core(mem, at, cap), EFAULT, at, cap);
    let Some(u) = queue().up.pop_front() else {
        return 0;
    };
    let fits = (WORD_BYTES as usize).saturating_add(u.pkt.len()) <= cap as usize;
    xpute_kit::ensure!(fits, EMSGSIZE, u.cmd, u.pkt.len());
    let f = Frame {
        tag: u.tag,
        cmd: u.cmd,
        flags: u.flags,
        result: u.result,
        pkt_len: u.pkt.len() as u32,
    };
    f.lay(mem, at);
    // SAFETY: the room the core named, which `fits` holds the packet.
    unsafe {
        core::ptr::copy_nonoverlapping(
            u.pkt.as_ptr(),
            mem.at::<u8>((at + WORD_BYTES) as usize),
            u.pkt.len(),
        );
    }
    1
}

/// The core's `send` (xpute-core host.rs), on the core's thread: copies the
/// frame at `at` and the memory it names, and queues them for this thread. A
/// frame or a packet that is not the core's, the frame at offset 0 among
/// them, and named memory that is not the core's (`in_core`), are the core's
/// bug.
pub(super) fn send(at: u32, named_at: u32, named_len: u32) {
    let mem = xpute_core::mem::core();
    xpute_kit::ensure!(in_core(mem, at, WORD_BYTES), EFAULT, at);
    let f = Frame::at(mem, at);
    let room = WORD_BYTES.saturating_add(f.pkt_len);
    xpute_kit::ensure!(mem.holds(at as usize, room as usize), EFAULT, at, f.pkt_len);
    xpute_kit::ensure!(
        in_core(mem, named_at, named_len),
        EFAULT,
        named_at,
        named_len
    );
    let pkt = f.pkt(mem, at, room).ok().flatten().unwrap_or_default();
    let down = Down {
        tag: f.tag,
        cmd: f.cmd,
        flags: f.flags,
        result: f.result,
        pkt: pkt.to_vec(),
        named: bytes_of(mem, named_at, named_len).to_vec(),
    };
    mark(|q| q.down.push(down));
}

fn serve(mem: Memory) {
    loop {
        let (sent, frame, waiting) = {
            let mut q = queue();
            while !q.dirty {
                q = CV.wait(q).unwrap_or_else(PoisonError::into_inner);
            }
            q.dirty = false;
            (replace_empty(&mut q.down), q.frame.take(), !q.up.is_empty())
        };
        // A grant goes on only while turns run, so none waits for a turn run
        // while paused.
        if let Some(grant) = frame.filter(|_| thread::running()) {
            queue().up.extend(granted(grant));
            thread::kick(grant.deadline_at);
        }
        // Whether the core has stopped is read before what it sent is served,
        // so that its report, sent before it stopped, is served first.
        let stopped = thread::terminated();
        with(|s| sent.into_iter().for_each(|f| take_frame(s, mem, f)));
        // What the UI sent and what was completed ask for a turn; a grant
        // brings its own (`kick`).
        if waiting {
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
    let frame = pkt(3, |w| {
        w.f64(Some(grant.refresh_ms))
            .f64(Some(grant.paced_ms))
            .bool(Some(grant.interactive));
    });
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    let turn = pkt(1, |w| {
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
/// own packets are a few numbers, far under `PKT_BYTES`.
fn pkt(cnt: u32, write: impl FnOnce(&mut PacketWriter)) -> Vec<u8> {
    let mut buf = [0u8; PKT_BYTES];
    let mut w = PacketWriter::new(&mut buf, cnt);
    write(&mut w);
    let len = w
        .finish()
        .unwrap_or_else(|e| xpute_kit::bug!(EOVERFLOW, e.errnum as u32));
    buf[..len as usize].to_vec()
}

/// Whether frames wait for the core's next turn.
pub(super) fn waiting() -> bool {
    !queue().up.is_empty()
}

/// A display frame's grant, from `vsync`. Before a runtime has attached it is
/// dropped, and the next frame grants again.
pub(super) fn frame(grant: Grant) {
    if STARTED.is_completed() {
        mark(|q| q.frame = Some(grant));
    }
}

fn down(s: &mut Shared, tag: u32, cmd: u32, flags: u32, result: i32, pkt: &[u8], named: &[u8]) {
    let mut header = [0u32; HEADER_WORDS as usize];
    for (k, w) in [
        (batch::TAG, tag),
        (batch::CMD, cmd),
        (batch::FLAGS, flags),
        (batch::RESULT, result as u32),
        (batch::PKT, pkt.len() as u32),
        (batch::NAMED, named.len() as u32),
    ] {
        header[k as usize] = w;
    }
    for w in header {
        s.down.extend_from_slice(&w.to_le_bytes());
    }
    for bytes in [pkt, named] {
        s.down.extend_from_slice(bytes);
        s.down
            .resize(s.down.len().next_multiple_of(ALIGN as usize), 0);
    }
}

/// Whether the `len` bytes at `at` are the core's to lend: inside its memory
/// and not at offset 0, which is no core's. No bytes lend nothing, wherever.
pub(super) fn in_core(mem: Memory, at: u32, len: u32) -> bool {
    len == 0 || (at != 0 && mem.holds(at as usize, len as usize))
}

fn bytes_of(mem: Memory, at: u32, len: u32) -> &'static [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the bytes a frame names, which the core holds still while they
    // are read (xpute-core io/door.rs R1); the memory lives as long as the
    // process.
    unsafe { core::slice::from_raw_parts(mem.at::<u8>(at as usize), len as usize) }
}

fn take_frame(s: &mut Shared, mem: Memory, f: Down) {
    if f.cmd == SysCommand::SYS_PACE as u32 {
        if let Ok(c) = SysPace::read(&f.pkt) {
            thread::ask(c.interval_ms);
        }
        return;
    }
    if f.flags & frame::RES == 0 && take_file(s, mem, &f) {
        return;
    }
    down(s, f.tag, f.cmd, f.flags, f.result, &f.pkt, &f.named);
}

/// Serves a frame of the file device, and says whether nothing more is to be
/// done with it.
fn take_file(s: &mut Shared, mem: Memory, f: &Down) -> bool {
    let pkt = f.pkt.as_slice();
    let Some(cmd) = FsCommand::of(f.cmd) else {
        return false;
    };
    if !s.fs {
        // A manifest with no file device: every frame of it is refused with
        // ENODEV, as the page's runtime refuses it (@xpute/runtime NO_FILES).
        complete(f.tag, f.cmd, Errno::ENODEV as i32, Vec::new());
        return true;
    }
    let fd = f.tag;
    match cmd {
        // What an open's path names is the program's. An open to write
        // reaches no binding before its close, so it is completed at once.
        FsCommand::FS_OPEN => {
            let Ok((path, flags)) = FsOpen::parts(pkt) else {
                complete_open(fd, Errno::EBADMSG as i32, 0);
                return true;
            };
            match flags {
                flag::O_RDONLY => {
                    s.files.insert(
                        fd,
                        File::Read {
                            bytes: None,
                            err: 0,
                        },
                    );
                    return false;
                }
                flag::O_WRONLY => {
                    let file = File::Write {
                        path: pkt_of_path(pkt, path),
                        bytes: Vec::new(),
                        closing: false,
                        err: 0,
                    };
                    s.files.insert(fd, file);
                    complete_open(fd, 0, 0);
                }
                // A refused open's close completes with its errno too, as no
                // read or write of it reaches a file.
                _ => {
                    let refused = File::Read {
                        bytes: Some(Vec::new()),
                        err: Errno::EINVAL as i32,
                    };
                    s.files.insert(fd, refused);
                    complete_open(fd, Errno::EINVAL as i32, 0);
                }
            }
            true
        }
        FsCommand::FS_READ => {
            let Ok(c) = FsRead::read(pkt) else {
                return true;
            };
            let Some(file) = s.files.get_mut(&fd) else {
                return true;
            };
            let File::Read {
                bytes: Some(bytes), ..
            } = file
            else {
                file.fail(Errno::EBADF);
                return true;
            };
            if !in_core(mem, c.at, c.len) {
                file.fail(Errno::EFAULT);
                return true;
            }
            let len = (c.len as usize).min(bytes.len());
            // SAFETY: the bytes the frame names, inside the core's memory and
            // the runtime's until the close of its descriptor is completed
            // (xpute-core io/door.rs R1).
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), mem.at::<u8>(c.at as usize), len);
            }
            true
        }
        // A write's bytes are held with its file, until its close.
        FsCommand::FS_WRITE => {
            match s.files.get_mut(&fd) {
                Some(File::Write { bytes, .. }) => bytes.extend_from_slice(&f.named),
                Some(file @ File::Read { .. }) => file.fail(Errno::EBADF),
                None => {}
            }
            true
        }
        FsCommand::FS_CLOSE => {
            close(s, fd);
            true
        }
        FsCommand::FS_CHANGED => false,
    }
}

/// Ends `fd`. The program is asked to commit a written file (`COMMIT`), and it
/// completes the close with the result (`post`); one that cannot be committed
/// is completed here with its errno. A file read is ended here and its close
/// completes with its errno; an open not yet completed is completed first with
/// ECANCELED, and the program is told to abort it (FS_CLOSE).
fn close(s: &mut Shared, fd: u32) {
    let close = FsCommand::FS_CLOSE as u32;
    match s.files.get_mut(&fd) {
        None => complete(fd, close, Errno::EBADF as i32, Vec::new()),
        Some(&mut File::Write { err, .. }) if err != 0 => {
            s.files.remove(&fd);
            complete(fd, close, err, Vec::new());
        }
        Some(File::Write {
            path,
            bytes,
            closing,
            ..
        }) => {
            *closing = true;
            let (path, bytes) = (core::mem::take(path), core::mem::take(bytes));
            down(s, fd, COMMIT, 0, 0, &path, &bytes);
        }
        Some(File::Read { bytes, err }) => {
            let (pending, err) = (bytes.is_none(), *err);
            s.files.remove(&fd);
            if pending {
                complete_open(fd, Errno::ECANCELED as i32, 0);
                down(s, fd, close, 0, 0, &[], &[]);
            }
            complete(fd, close, err, Vec::new());
        }
    }
}

fn complete(tag: u32, cmd: u32, result: i32, pkt: Vec<u8>) {
    mark(|q| {
        q.up.push_back(Up {
            tag,
            cmd,
            flags: frame::RES,
            result,
            pkt,
        });
    });
}

fn complete_open(fd: u32, result: i32, detail: u32) {
    let pkt = pkt(1, |w| {
        w.u32(Some(detail));
    });
    complete(fd, FsCommand::FS_OPEN as u32, result, pkt);
}

/// A packet of `path` alone, as COMMIT carries it, read from the open's
/// packet `pkt`, which holds it and more.
fn pkt_of_path(pkt: &[u8], path: Path) -> Vec<u8> {
    let tokens: Vec<_> = path.iter().collect();
    let mut buf = vec![0u8; pkt.len()];
    let mut w = PacketWriter::new(&mut buf, 1);
    Path::write(&mut w, &tokens);
    let len = w
        .finish()
        .unwrap_or_else(|e| xpute_kit::bug!(EOVERFLOW, e.errnum as u32));
    buf.truncate(len as usize);
    buf
}

/// The program's bytes for the open of `fd`, or its refusal: a negative `result`
/// and the number the store refused with.
pub fn fs_complete_open(fd: u32, result: i32, detail: u32, bytes: Vec<u8>) {
    with(|s| {
        let Some(File::Read {
            bytes: held @ None, ..
        }) = s.files.get_mut(&fd)
        else {
            return;
        };
        if result < 0 {
            s.files.remove(&fd);
        } else {
            *held = Some(bytes);
        }
        complete_open(fd, result, detail);
    });
}

/// Queues a frame for the core. The program's completion of a written file's
/// close lets the file go.
pub fn post(tag: u32, cmd: u32, flags: u32, result: i32, pkt: &[u8]) {
    if cmd == FsCommand::FS_CLOSE as u32 && flags & frame::RES != 0 {
        with(|s| {
            if let Some(File::Write { closing: true, .. }) = s.files.get(&tag) {
                s.files.remove(&tag);
            }
        });
    }
    let up = Up {
        tag,
        cmd,
        flags,
        result,
        pkt: pkt.to_vec(),
    };
    mark(|q| {
        q.up.push_back(up);
    });
}

/// Hands over what the UI has not heard, as a batch (spec/io/batch.json).
pub fn take() -> Vec<u8> {
    with(|s| {
        s.told = false;
        replace_empty(&mut s.down)
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "serve.test.rs"]
mod test;
