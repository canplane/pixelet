// xpute-core/ipc/door.rs

//! The door is the core's end of its two rings. The runtime submits
//! commands on the submission ring, and the door applies them in order. The
//! core posts its replies and its own commands on the completion ring,
//! writing each frame in place. The runtime reads the completion ring empty
//! before it rings the core again, so overrunning that ring is a sizing bug,
//! not a state to wait out.
//!
//! A command under xpute's majors (abi/sys.json, abi/gpu.json, abi/fs.json) is
//! applied here, or handed to the program as what it means. The program is
//! handed every other command, and each open and close the runtime has
//! completed.
//!
//! The core's imports, the commands it sends the runtime, are frames too. Its
//! file calls (abi/fs.json) and its stop report are posted as any import is.
//! An open and a close carry ACKREQ, and the runtime completes each on a later
//! turn with a frame flagged RES on the submission ring. A call, which is
//! answered before it returns, is laid unpublished in the next slot, and its
//! place is handed to the runtime's `call`. The runtime routes each import by
//! its command's major.
//!
//! Both rings keep the rules of a ring (ring.rs R1 to R3), so the door works
//! whether its runtime takes turns with the core or runs beside it. A
//! runtime beside the core begins a turn only while the completion ring has
//! room for one (`has_room`). The door keeps R3 in its own terms, and two
//! rules more, both io_uring's as well:
//!
//! - R3, as the door keeps it: memory that an import's frame names by its
//!   address is the runtime's until the runtime's head has passed the frame
//!   (`last_pos`, `is_consumed`). Only then does the core write that memory
//!   again, or free it. Memory a call names is the runtime's only until the
//!   call returns. The bytes an FS_WRITE names are the runtime's until the
//!   FS_CLOSE of their descriptor is completed.
//! - R4. A completion finds its request by its tag, never by where it lies
//!   in either ring or when it came.
//! - R5. A call never nests: the runtime answers it without calling into the
//!   core, and the core makes no call while another is unanswered.
//!
//! The last slot of the completion ring is reserved for the stop report, so
//! a core that stops can say why without taking back a frame it has
//! published.

use core::cell::{Cell, RefCell};
use core::panic::Location;
use std::rc::Rc;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::PacketWriter;

use crate::abi::fs::{self, FsChanged, FsClose, FsOpenat, FsOpenatReply, FsRead, FsWrite};
use crate::abi::gpu::{self, GpuSurfaceClose, GpuSurfaceResize};
use crate::abi::sys::{Command, Nop, SysAbort, SysTurn};
use crate::call::answer_of;
use crate::ipc::frame;
use crate::ipc::fs::{Fd, FsClosed, FsOpened};
use crate::ipc::ring::Ring;
use crate::mem::off_of_slice;
use crate::sched::wake::NO_WAKE;

/// An import is what xpute-tool generates from a protocol's table (`spec/cmd`):
/// its command number, the number of children its packet is written as, and its
/// writer.
pub trait Import {
    const NUMBER: u32;
    const FIELDS: u32;
    fn write(&self, w: &mut PacketWriter);
}

/// How many bytes from the end of a stop report's file path are kept when
/// the whole path would not fit in a slot.
const ABORT_PATH_BYTES: usize = 32;

/// Where an import's frame was posted in the completion ring, for asking
/// `is_consumed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos(u32);

/// A read into a buffer the door holds (`Door::read_buf`). The bytes are
/// handed over once the runtime's head has passed the read. A reading dropped
/// before then leaves its buffer with the door, which lets the buffer go
/// once the head has passed.
pub struct Reading {
    pos: Pos,
    buf: Rc<Vec<u8>>,
}

impl Reading {
    /// Answers the bytes once the runtime's head has passed the read. Before
    /// then it gives the reading back untouched.
    pub fn take(self, door: &Door) -> Result<Vec<u8>, Reading> {
        if !door.is_consumed(self.pos) {
            return Err(self);
        }
        door.reads.borrow_mut().retain(|(pos, _)| *pos != self.pos);
        Ok(Rc::try_unwrap(self.buf).or_bug(Errno::ENOTRECOVERABLE))
    }
}

/// What the program answers a submission: a value to reply with, or none for
/// a command it does not know, which is replied to with ENOSYS.
pub type Outcome = Result<Option<i32>, MarshalError>;

/// The door hands the program a change in its life that its runtime tells it
/// of, a turn the scheduling policy grants, the GPU's answer, a surface the UI
/// lends it, an open or a close the runtime has completed, a file changed
/// under it, or a command of the program's own, with its number and its
/// packet. Nothing answers a completion, so what the program returns for one
/// is dropped. The drain runs before any phase's budget, so the program should
/// record what it is handed and return. xpute may add kinds: a program matches
/// the kinds it takes and leaves the rest to a wildcard.
#[non_exhaustive]
pub enum Submission<'a> {
    Life(Life),
    /// The scheduling policy grants a turn (SYS_TURN), which carries the wall
    /// clock.
    Turn(SysTurn),
    /// The GPU the machine asked for has answered (GPU_READY), for the first
    /// time or again after a loss. It tells of the GPU's state, not of the
    /// core's life.
    GpuReady,
    Surface(Surface),
    Opened(FsOpened),
    Closed(FsClosed),
    /// A file changed under the core, so what the core read of it is stale
    /// (FS_CHANGED).
    Changed(FsChanged<'a>),
    Program(u32, Option<&'a [u8]>),
}

/// A surface is lent to the GPU by the UI (abi/gpu.json). It is opened, and the
/// program answers with its handle, or 0 for none; its box is resized; it is
/// let go.
#[derive(Clone, Copy, Debug)]
pub enum Surface {
    Open,
    Resize(GpuSurfaceResize),
    Close(GpuSurfaceClose),
}

/// The core's life is as its runtime tells it (abi/sys.json). What each event
/// means the program keeps or resets is the program's to decide; a program
/// with nothing to do answers `Ok(None)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// A runtime has attached, for the first time or again (SYS_ATTACH).
    Attach,
    /// The core is about to be frozen; this is its last sure moment to save
    /// (SYS_SUSPEND).
    Suspend,
    /// The core has been woken (SYS_RESUME).
    Resume,
}

/// A stat says how full the rings ran since the stat was last taken
/// (`Door::take_stat`). Each figure is the most it reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoorStat {
    pub cq_peak: u32,
    /// The submission ring's length when a drain started, which shows how
    /// close it came to full.
    pub sq_peak: u32,
    /// The submissions a drain left unapplied for lack of room.
    pub left_peak: u32,
}

pub struct Door {
    sq: Ring,
    cq: Ring,
    /// The completion slots that applying one command may need: its imports,
    /// or the turn's imports after the drain, plus its reply and the stop
    /// report's slot.
    reserve: u32,
    stat: Cell<DoorStat>,
    /// Each write's bytes, kept until the FS_CLOSE of the descriptor its
    /// `write` named is completed.
    writes: RefCell<Vec<(Fd, Vec<u8>)>>,
    /// Each read's buffer, kept until the runtime's head has passed the
    /// `read_buf` that names it, whether or not its reading is still held.
    reads: RefCell<Vec<(Pos, Rc<Vec<u8>>)>>,
}

impl Door {
    /// `most_frames` is the program's bound on the frames one command posts,
    /// or the turn posts once its drain is done.
    pub fn new(sq: Ring, cq: Ring, most_frames: u32) -> Door {
        Door {
            sq,
            cq,
            reserve: most_frames + 2,
            stat: Cell::default(),
            writes: RefCell::new(Vec::new()),
            reads: RefCell::new(Vec::new()),
        }
    }

    /// `rise` begins a turn. The door lets go of every read the runtime's head
    /// has passed; the buffer itself is freed only where its reading was
    /// dropped.
    pub fn rise(&self) {
        self.reads
            .borrow_mut()
            .retain(|&(pos, _)| !self.is_consumed(pos));
    }

    pub fn last_pos(&self) -> Pos {
        Pos(self.cq.tail().wrapping_sub(1))
    }

    /// Whether the runtime's head has passed the import posted at `pos`, so
    /// that the memory it named is the core's again (R3).
    pub fn is_consumed(&self, pos: Pos) -> bool {
        self.cq.is_consumed(pos.0)
    }

    pub fn take_stat(&self) -> DoorStat {
        self.stat.take()
    }

    fn keep(&self, f: impl FnOnce(&mut DoorStat)) {
        let mut s = self.stat.get();
        f(&mut s);
        self.stat.set(s);
    }

    pub fn sq(&self) -> &Ring {
        &self.sq
    }

    pub fn cq(&self) -> &Ring {
        &self.cq
    }

    pub fn post(&self, tag: u32, cmd: u32, flags: u32, result: i32) {
        let e = match self.can_post() {
            true => self.cq.push(tag, cmd, flags, result, None),
            false => Errno::EAGAIN,
        };
        self.check_post(cmd, e);
    }

    /// `write` fills the slot's payload and answers the packet's length, or
    /// none when it did not fit.
    pub fn post_in_place(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        write: impl FnOnce(&mut [u8]) -> Option<u32>,
    ) {
        let e = match self.can_post() {
            true => self.cq.push_in_place(tag, cmd, flags, result, write),
            false => Errno::EAGAIN,
        };
        self.check_post(cmd, e);
    }

    /// Whether a frame can be laid and the stop report's slot still be free.
    fn can_post(&self) -> bool {
        self.cq.len() + 1 < self.cq.cap
    }

    fn check_post(&self, cmd: u32, e: Errno) {
        self.keep(|s| s.cq_peak = s.cq_peak.max(self.cq.len()));
        xpute_kit::ensure!(e == Errno::OK, ENOSPC, cmd, e as i32);
    }

    /// Posts a frame tagged `tag` and flagged `flags`, whose packet of `cnt`
    /// children `f` writes.
    pub fn send_with(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        cnt: u32,
        f: impl Fn(&mut PacketWriter),
    ) {
        self.post_in_place(tag, cmd, flags, 0, |out: &mut [u8]| {
            let mut w = PacketWriter::new(out, cnt);
            f(&mut w);
            w.fit_len()
        });
    }

    pub fn send<E: Import>(&self, import: &E) {
        self.send_with(0, E::NUMBER, 0, E::FIELDS, |w| import.write(w));
    }

    /// `call` sends `import`, which the runtime answers before this returns.
    /// The frame is laid unpublished in the next slot and handed to the
    /// runtime's `call`. It answers the runtime's value or errno, ENOSPC when
    /// no slot is free, or EMSGSIZE when the frame does not fit in one.
    pub fn call<E: Import>(&self, import: &E) -> Result<u32, Errno> {
        if !self.can_post() {
            return Err(Errno::ENOSPC);
        }
        let at = self
            .cq
            .stage_in_place(0, E::NUMBER, frame::ACKREQ, |out| {
                let mut w = PacketWriter::new(out, E::FIELDS);
                import.write(&mut w);
                w.fit_len()
            })
            .map_err(|e| if e == Errno::EAGAIN { Errno::ENOSPC } else { e })?;
        answer_of(crate::call::call(at as u32))
    }

    /// Opens the file at `path` below `root` as `fd`. The runtime completes
    /// the open on a later turn (`Submission::Opened`).
    pub fn openat(&self, fd: Fd, root: u32, flags: u32, path: &str) {
        let import = FsOpenat {
            fd,
            root,
            flags,
            path: &path,
        };
        self.send_with(fd, FsOpenat::NUMBER, frame::ACKREQ, FsOpenat::FIELDS, |w| {
            import.write(w)
        });
    }

    /// Reads into `dst`, which is filled once the runtime's head has passed
    /// the import. Answers where the import was posted.
    ///
    /// # Safety
    /// `dst` must stay allocated, unmoved and unread until `is_consumed` says
    /// the runtime has passed the import.
    pub unsafe fn read(&self, fd: Fd, dst: &mut [u8]) -> Pos {
        let at = off_of_slice(&raw mut *dst) as u32;
        self.send(&FsRead {
            fd,
            at,
            len: dst.len() as u32,
        });
        self.last_pos()
    }

    /// Reads into `buf`, which is filled once the runtime's head has passed
    /// the import and is the door's until then.
    pub fn read_buf(&self, fd: Fd, mut buf: Vec<u8>) -> Reading {
        // SAFETY: the buffer moves into the door, which neither reads, grows
        // nor drops it before `is_consumed` says the runtime is done with it.
        let pos = unsafe { self.read(fd, &mut buf) };
        let buf = Rc::new(buf);
        self.reads.borrow_mut().push((pos, Rc::clone(&buf)));
        Reading { pos, buf }
    }

    /// Writes `bytes`, which are the door's until `fd`'s close is completed.
    pub fn write(&self, fd: Fd, bytes: Vec<u8>) {
        let at = off_of_slice(bytes.as_slice()) as u32;
        self.send(&FsWrite {
            fd,
            at,
            len: bytes.len() as u32,
        });
        self.writes.borrow_mut().push((fd, bytes));
    }

    /// Closes `fd`, which commits a write and completes an open not yet
    /// completed with ECANCELED. The runtime completes the close on a later
    /// turn (`Submission::Closed`).
    pub fn close(&self, fd: Fd) {
        let import = FsClose { fd };
        self.send_with(fd, FsClose::NUMBER, frame::ACKREQ, FsClose::FIELDS, |w| {
            import.write(w)
        });
    }

    /// Posts the stop report as the core stops, after what the core already
    /// published, in the slot kept for it. Where the whole report would not
    /// fit in a slot, its file path is cut to its last bytes, or left out.
    pub fn abort(&self, errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
        let path = at.file();
        let tail = |n: usize| path.get(path.len().saturating_sub(n)..).unwrap_or("");
        for file in [path, tail(ABORT_PATH_BYTES), ""] {
            let report = SysAbort {
                errnum: errnum as i32,
                line: at.line(),
                column: at.column(),
                a_low: a as u32,
                a_high: (a >> 32) as u32,
                b_low: b as u32,
                b_high: (b >> 32) as u32,
                file: &file,
            };
            let e = self.cq.push_in_place(0, SysAbort::NUMBER, 0, 0, |out| {
                let mut w = PacketWriter::new(out, SysAbort::FIELDS);
                report.write(&mut w);
                w.fit_len()
            });
            if e != Errno::EMSGSIZE || file.is_empty() {
                return self.check_post(SysAbort::NUMBER, e);
            }
        }
    }

    /// Whether the completion ring has room for what one more command may
    /// post. An unapplied command can wait in the submission ring, but what it
    /// posts could wait nowhere, so room is checked before a command starts.
    /// A runtime that runs beside the core checks it before it begins a turn.
    pub fn has_room(&self) -> bool {
        self.cq.len() + self.reserve <= self.cq.cap
    }

    /// `turn` is a turn's drain. It lets go of the reads the runtime has passed
    /// (`rise`), hands what waits to `handle` in order (`receive`), and answers
    /// when the core wants the next turn: at once when submissions are left,
    /// else only when something arrives.
    pub fn turn(&self, handle: impl FnMut(Submission) -> Outcome) -> f64 {
        self.rise();
        self.receive(handle);
        if self.pending() {
            0.0
        } else {
            NO_WAKE
        }
    }

    pub fn pending(&self) -> bool {
        !self.sq.is_empty()
    }

    /// Applies submissions in order while there is room; the rest wait for
    /// the next turn. It replies only to frames flagged `ACKREQ` and to
    /// failures, and never to a completion. Answers how many frames it posted
    /// on the completion ring.
    pub fn receive(&self, mut handle: impl FnMut(Submission) -> Outcome) -> u32 {
        let tail = self.cq.tail();
        self.keep(|s| s.sq_peak = s.sq_peak.max(self.sq.len()));
        while self.has_room() {
            let Some(f) = self.sq.peek() else {
                break;
            };
            if f.flags & frame::RES != 0 {
                let pkt = self.sq.pkt(&f).ok().flatten();
                self.complete(&f, pkt, &mut handle);
                self.sq.advance();
                continue;
            }
            let result = match self.sq.pkt(&f) {
                Ok(pkt) => match apply_frame(&f, pkt, &mut handle) {
                    Ok(Some(val)) => Ok(val),
                    Ok(None) => Err(Errno::ENOSYS as i32),
                    Err(e) => Err(e.errnum as i32),
                },
                Err(e) => Err(e.errnum as i32),
            };
            self.sq.advance();
            if f.flags & frame::ACKREQ != 0 || result.is_err() {
                self.post(
                    f.tag,
                    f.cmd,
                    frame::RES,
                    result.unwrap_or_else(|errnum| errnum),
                );
            }
        }
        self.keep(|s| s.left_peak = s.left_peak.max(self.sq.len()));
        self.cq.tail().wrapping_sub(tail)
    }

    /// Handles the runtime's completion of an open or a close, found by its
    /// tag, which is the descriptor (R4). A close's completion lets go of the
    /// bytes the descriptor's writes named. Any other completion is dropped.
    fn complete(
        &self,
        f: &frame::Frame,
        pkt: Option<&[u8]>,
        handle: &mut impl FnMut(Submission) -> Outcome,
    ) {
        let fd = f.tag;
        match fs::Command::of(f.cmd) {
            Some(fs::Command::FS_OPENAT) => {
                let detail = pkt
                    .and_then(|pkt| FsOpenatReply::read(pkt).ok())
                    .map_or(0, |r| r.detail);
                let _ = handle(Submission::Opened(FsOpened {
                    fd,
                    res: f.result,
                    detail,
                }));
            }
            Some(fs::Command::FS_CLOSE) => {
                self.writes.borrow_mut().retain(|&(of, _)| of != fd);
                let _ = handle(Submission::Closed(FsClosed { fd, res: f.result }));
            }
            _ => {}
        }
    }
}

/// A command with fields must carry a packet.
fn packet(pkt: Option<&[u8]>) -> Result<&[u8], MarshalError> {
    pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))
}

/// Applies one submitted command. Every command past xpute's majors goes to
/// the program. Of xpute's own, NOP answers its value and the rest are handed
/// over as what they mean; any other of xpute's numbers is answered with
/// nothing.
fn apply_frame(
    f: &frame::Frame,
    pkt: Option<&[u8]>,
    handle: &mut impl FnMut(Submission) -> Outcome,
) -> Outcome {
    if f.cmd >> 8 >= frame::PROGRAM_MAJOR {
        return handle(Submission::Program(f.cmd, pkt));
    }
    match Command::of(f.cmd) {
        Some(Command::NOP) => Ok(Some(match pkt {
            Some(pkt) => Nop::read(pkt)?.val as i32,
            None => 0,
        })),
        Some(Command::SYS_ATTACH) => handle(Submission::Life(Life::Attach)),
        Some(Command::SYS_SUSPEND) => handle(Submission::Life(Life::Suspend)),
        Some(Command::SYS_RESUME) => handle(Submission::Life(Life::Resume)),
        Some(Command::SYS_TURN) => handle(Submission::Turn(SysTurn::read(packet(pkt)?)?)),
        // The core's own report, which the runtime never sends.
        Some(Command::SYS_ABORT) => Ok(None),
        None => match (gpu::Command::of(f.cmd), fs::Command::of(f.cmd)) {
            (Some(gpu::Command::GPU_READY), _) => handle(Submission::GpuReady),
            (Some(gpu::Command::GPU_SURFACE_OPEN), _) => handle(Submission::Surface(Surface::Open)),
            (Some(gpu::Command::GPU_SURFACE_RESIZE), _) => handle(Submission::Surface(
                Surface::Resize(GpuSurfaceResize::read(packet(pkt)?)?),
            )),
            (Some(gpu::Command::GPU_SURFACE_CLOSE), _) => handle(Submission::Surface(
                Surface::Close(GpuSurfaceClose::read(packet(pkt)?)?),
            )),
            (_, Some(fs::Command::FS_CHANGED)) => {
                handle(Submission::Changed(FsChanged::read(packet(pkt)?)?))
            }
            // The core's file calls, which the runtime never sends, and the
            // numbers xpute gives to no command.
            _ => Ok(None),
        },
    }
}

#[cfg(test)]
#[path = "door.test.rs"]
mod test;
