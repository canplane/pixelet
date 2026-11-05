// xpute-guest/ipc/door.rs

//! The guest's end of its rings: submissions applied in order, completions
//! posted in place. The host reads the completion ring dry before it rings
//! again, so overrunning it is a sizing bug, not a state to wait out. A
//! command under xpute's majors (abi/cmd.json, abi/fs.json) is applied here
//! or handed over as what it means; the program is handed every other, and
//! each open and close the host completed.
//!
//! The guest's hostcalls are frames too: its files (abi/fs.json) and its
//! report are posted as any event is, an open and a close with ACKREQ, which
//! the host completes on a later turn with a frame flagged RES on the
//! submission ring; a call answered before it returns is laid unpublished in
//! the next slot and its place handed to the host's `hostcall`. The host
//! routes each by its command's major.
//!
//! Both rings keep a ring's rules (ring.rs R1 to R3), so the door holds
//! whether its host takes turns with the guest or runs beside it, a host
//! beside it beginning a turn only while the completion ring has room for
//! one (`has_room`); and two more, io_uring's too:
//!
//! - R3, as the door keeps it: memory an event names by its address is the
//!   host's until the host's head has passed the event (`last_pos`,
//!   `is_consumed`), so the guest writes it again, or frees it, only then.
//!   Memory a call names is the host's only until the call returns, and the
//!   bytes an FS_WRITE names are the host's until its descriptor's FS_CLOSE
//!   is completed.
//! - R4. A completion finds its request by its tag, never by where it lies
//!   in either ring or when it came.
//! - R5. A call never nests: the host answers it without calling into the
//!   guest, and the guest makes no call while another is unanswered.
//!
//! The last slot of the completion ring is the stop report's alone, so a
//! guest that stops can say why without taking back a frame it published.

use core::cell::{Cell, RefCell};
use core::panic::Location;
use std::rc::Rc;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::PacketWriter;

use crate::abi::cmd::{Command, Nop, SysAbortEvent};
use crate::abi::fs::{self, FsCloseEvent, FsOpenatEvent, FsOpenatReply, FsReadEvent, FsWriteEvent};
use crate::hostcall::answer_of;
use crate::ipc::frame;
use crate::ipc::fs::{Fd, FsClosed, FsOpened};
use crate::ipc::ring::Ring;
use crate::mem::off_of_slice;

/// An event a table generates (xpute-tool `spec/cmd`): its number, the
/// children it is written as, and its writer.
pub trait Event {
    const NUMBER: u32;
    const FIELDS: u32;
    fn write(&self, w: &mut PacketWriter);
}

/// How much of a report's path is kept where the whole would not fit a slot.
const ABORT_PATH_BYTES: usize = 32;

/// Where an event was posted in the completion ring: asked of `is_consumed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos(u32);

/// A read into a buffer the door holds (`Door::read_buf`): the bytes are
/// handed over once the host's head has passed the read. A reading dropped
/// before then leaves its buffer with the door, which lets it go once the
/// head has passed.
pub struct Reading {
    pos: Pos,
    buf: Rc<Vec<u8>>,
}

impl Reading {
    /// The bytes, once the host's head has passed the read; the reading
    /// again, untouched, before then.
    pub fn take(self, door: &Door) -> Result<Vec<u8>, Reading> {
        if !door.is_consumed(self.pos) {
            return Err(self);
        }
        door.reads
            .borrow_mut()
            .retain(|(pos, _)| *pos != self.pos);
        Ok(Rc::try_unwrap(self.buf).or_bug(Errno::ENOTRECOVERABLE))
    }
}

/// None for an unknown command, which replies ENOSYS.
pub type Outcome = Result<Option<i32>, MarshalError>;

/// What the door hands the program: a turn of its life its host told it, its
/// GPU grant answered, an open or a close the host completed, or a command of
/// the program's own, its number and its packet. A completion is answered by
/// nothing, so what the program returns for one is dropped. A drain runs before any phase's
/// budget, so the program records what it is handed and returns.
pub enum Submission<'a> {
    Life(Life),
    /// The GPU the machine asked for answered, the first time or again after
    /// a loss (GPU_READY): the GPU grant's state, not the guest's life.
    GpuReady,
    Opened(FsOpened),
    Closed(FsClosed),
    Program(u32, Option<&'a [u8]>),
}

/// The guest's life as its host tells it (abi/cmd.json): what each means to
/// keep or reset is the program's, and a program with nothing to do answers
/// `Ok(None)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Life {
    /// A host attached, the first or again (SYS_ATTACH).
    Attach,
    /// About to be frozen: the last sure moment to save (SYS_SUSPEND).
    Suspend,
    /// Woken (SYS_RESUME).
    Resume,
}

/// How full the rings ran since the stat was last taken (`Door::take_stat`):
/// each figure the most it reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoorStat {
    pub cq_peak: u32,
    /// The submission ring's length as a drain started: how close it came to full.
    pub sq_peak: u32,
    /// The submissions a drain left for lack of room.
    pub left_peak: u32,
}

pub struct Door {
    sq: Ring,
    cq: Ring,
    /// What one command may leave: its events, or the turn's after the drain,
    /// its reply and the stop report's slot.
    reserve: u32,
    stat: Cell<DoorStat>,
    /// Kept until the FS_CLOSE of the descriptor its `write` named is completed.
    writes: RefCell<Vec<(Fd, Vec<u8>)>>,
    /// Kept until the host's head has passed the `read_buf` that names it,
    /// whether or not its reading is still held.
    reads: RefCell<Vec<(Pos, Rc<Vec<u8>>)>>,
}

impl Door {
    /// `most_events` is the most one command raises, or the turn once its
    /// drain is done: the program's bound.
    pub fn new(sq: Ring, cq: Ring, most_events: u32) -> Door {
        Door {
            sq,
            cq,
            reserve: most_events + 2,
            stat: Cell::default(),
            writes: RefCell::new(Vec::new()),
            reads: RefCell::new(Vec::new()),
        }
    }

    /// A turn begins: what the reads the host has taken named is let go,
    /// only where its reading was dropped.
    pub fn rise(&self) {
        self.reads
            .borrow_mut()
            .retain(|&(pos, _)| !self.is_consumed(pos));
    }

    /// The event posted last.
    pub fn last_pos(&self) -> Pos {
        Pos(self.cq.tail().wrapping_sub(1))
    }

    /// Whether the host's head has passed the event posted at `pos`: what
    /// it named is the guest's again (R3).
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

    /// A frame tagged `tag` and flagged `flags`, its packet written by `f`.
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

    /// `event`, tagged 0.
    pub fn send<E: Event>(&self, event: &E) {
        self.send_with(0, E::NUMBER, 0, E::FIELDS, |w| event.write(w));
    }

    /// `event`, answered before this returns: laid in the next slot
    /// unpublished and handed to the host's `hostcall`, its value or its
    /// errno; ENOSPC where no slot is free, EMSGSIZE where it does not fit one.
    pub fn call<E: Event>(&self, event: &E) -> Result<u32, Errno> {
        if !self.can_post() {
            return Err(Errno::ENOSPC);
        }
        let at = self
            .cq
            .stage_in_place(0, E::NUMBER, frame::ACKREQ, |out| {
                let mut w = PacketWriter::new(out, E::FIELDS);
                event.write(&mut w);
                w.fit_len()
            })
            .map_err(|e| if e == Errno::EAGAIN { Errno::ENOSPC } else { e })?;
        answer_of(crate::hostcall::hostcall(at as u32))
    }

    /// The file at `path` below `root` opened as `fd`, completed on a later
    /// turn (`Submission::Opened`).
    pub fn openat(&self, fd: Fd, root: u32, flags: u32, path: &str) {
        let event = FsOpenatEvent {
            fd,
            root,
            flags,
            path: &path,
        };
        self.send_with(fd, FsOpenatEvent::NUMBER, frame::ACKREQ, FsOpenatEvent::FIELDS, |w| {
            event.write(w)
        });
    }

    /// `dst` is filled once the host's head has passed the event: where it
    /// was posted.
    ///
    /// # Safety
    /// `dst` stays allocated, unmoved and unread until `is_consumed` says so.
    pub unsafe fn read(&self, fd: Fd, dst: &mut [u8]) -> Pos {
        let at = off_of_slice(&raw mut *dst) as u32;
        self.send(&FsReadEvent {
            fd,
            at,
            len: dst.len() as u32,
        });
        self.last_pos()
    }

    /// `buf` is filled once the host's head has passed the event, and is
    /// the door's until then.
    pub fn read_buf(&self, fd: Fd, mut buf: Vec<u8>) -> Reading {
        // SAFETY: the buffer moves into the door, which neither reads, grows
        // nor drops it before `is_consumed` says the host is done with it.
        let pos = unsafe { self.read(fd, &mut buf) };
        let buf = Rc::new(buf);
        self.reads.borrow_mut().push((pos, Rc::clone(&buf)));
        Reading { pos, buf }
    }

    /// `bytes` are the door's until `fd`'s close is completed.
    pub fn write(&self, fd: Fd, bytes: Vec<u8>) {
        let at = off_of_slice(bytes.as_slice()) as u32;
        self.send(&FsWriteEvent {
            fd,
            at,
            len: bytes.len() as u32,
        });
        self.writes.borrow_mut().push((fd, bytes));
    }

    /// Commits a write, and completes an open not yet completed with
    /// ECANCELED; completed on a later turn (`Submission::Closed`).
    pub fn close(&self, fd: Fd) {
        let event = FsCloseEvent { fd };
        self.send_with(fd, FsCloseEvent::NUMBER, frame::ACKREQ, FsCloseEvent::FIELDS, |w| {
            event.write(w)
        });
    }

    /// Posts the report after what the guest already published, as the guest
    /// stops, in the slot kept for it: its file's path cut to its end, or left
    /// out, where the whole of it would not fit a slot.
    pub fn abort(&self, errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
        let path = at.file();
        let tail = |n: usize| path.get(path.len().saturating_sub(n)..).unwrap_or("");
        for file in [path, tail(ABORT_PATH_BYTES), ""] {
            let report = SysAbortEvent {
                errnum: errnum as i32,
                line: at.line(),
                column: at.column(),
                a_low: a as u32,
                a_high: (a >> 32) as u32,
                b_low: b as u32,
                b_high: (b >> 32) as u32,
                file: &file,
            };
            let e = self
                .cq
                .push_in_place(0, SysAbortEvent::NUMBER, 0, 0, |out| {
                    let mut w = PacketWriter::new(out, SysAbortEvent::FIELDS);
                    report.write(&mut w);
                    w.fit_len()
                });
            if e != Errno::EMSGSIZE || file.is_empty() {
                return self.check_post(SysAbortEvent::NUMBER, e);
            }
        }
    }

    /// An unapplied command can wait in the submission ring; what it would
    /// raise could not wait anywhere, so room is checked before it starts,
    /// and before a turn does, by a host that runs beside the guest.
    pub fn has_room(&self) -> bool {
        self.cq.len() + self.reserve <= self.cq.cap
    }

    pub fn pending(&self) -> bool {
        !self.sq.is_empty()
    }

    /// Applies submissions in order while there is room; the rest waits for
    /// the next turn. Replies only to `ACKREQ` and to failures, and to no
    /// completion. Returns how many completions it made.
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

    /// The host's completion of an open or a close, found by its tag, the
    /// descriptor (R4): a close's lets go of the bytes the descriptor's
    /// writes named. Any other is dropped.
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
                let _ = handle(Submission::Closed(FsClosed {
                    fd,
                    res: f.result,
                }));
            }
            _ => {}
        }
    }
}

/// xpute's commands applied, NOP answering its value and the guest's life
/// and its GPU grant's answer handed over, and any other of xpute's numbers
/// refused; every command past xpute's majors handed to
/// the program.
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
        Some(Command::GPU_READY) => handle(Submission::GpuReady),
        // The guest's own report and file calls, which the host never sends,
        // and the numbers xpute gives nothing.
        Some(Command::SYS_ABORT) | None => Ok(None),
    }
}

#[cfg(test)]
#[path = "door.test.rs"]
mod test;
