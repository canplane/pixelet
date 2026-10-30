// xpute-guest/ipc/sys.rs

//! A guest's hostcalls, recorded in a stream the host runs after the turn
//! returns; anything a call names must live until the next rise. The guest
//! issues its own descriptors as handles, so a late answer to a closed one
//! names nothing. `openat` is answered by SYS_OPENED on a later turn; a
//! program's call made as a `hostcall`, before it returns.

use core::panic::Location;

use xpute_kit::status::errno::Errno;

use crate::ipc::stream::{record_bytes, Stream};
use crate::mem::{off_of_slice, Memory};

#[path = "sys.spec.rs"]
pub mod spec;

pub type Fd = u32;

// Each call's arguments, which its record takes as an array of this length.
pub const OPENAT_ARGS: usize = spec::args::OPENAT as usize;
pub const READ_ARGS: usize = spec::args::READ as usize;
pub const WRITE_ARGS: usize = spec::args::WRITE as usize;
pub const CLOSE_ARGS: usize = spec::args::CLOSE as usize;
const ABORT_ARGS: usize = spec::args::ABORT as usize;

/// The most one descriptor records in a turn, its name `path` bytes long: its
/// open, a read or a write, and its close, twice where it is stopped.
pub const fn fd_turn_bytes(path: usize) -> usize {
    let moved = if READ_ARGS > WRITE_ARGS {
        READ_ARGS
    } else {
        WRITE_ARGS
    };
    record_bytes(OPENAT_ARGS, Some(path))
        + record_bytes(moved, None)
        + 2 * record_bytes(CLOSE_ARGS, None)
}

pub struct Sys {
    stream: Stream,
    /// Kept until the host has run the `write` that names it.
    held: Vec<Vec<u8>>,
}

impl Sys {
    /// # Safety
    /// As `Stream::new`.
    pub unsafe fn new(mem: Memory, at: usize, words: usize) -> Sys {
        Sys {
            // SAFETY: the caller's range.
            stream: unsafe { Stream::new(mem, at, words, spec::HEAD as usize) },
            held: Vec::new(),
        }
    }

    pub fn rise(&mut self) {
        self.held.clear();
    }

    pub fn fall(&mut self) {
        self.stream.publish();
    }

    pub fn openat(&mut self, fd: Fd, root: u32, flags: u32, path: &[u8]) {
        let args: [u32; OPENAT_ARGS] = [fd, root, flags];
        self.stream.record(spec::op::OPENAT, &args, &[], Some(path));
    }

    /// `dst` is filled once the host runs the turn's records.
    ///
    /// # Safety
    /// `dst` stays allocated, unmoved and unread until the next rise.
    pub unsafe fn read(&mut self, fd: Fd, dst: &mut [u8]) {
        let args: [u32; READ_ARGS] = [fd, off_of_slice(&raw mut *dst) as u32, dst.len() as u32];
        self.stream.record(spec::op::READ, &args, &[], None);
    }

    pub fn write(&mut self, fd: Fd, bytes: Vec<u8>) {
        let args: [u32; WRITE_ARGS] = [
            fd,
            off_of_slice(bytes.as_slice()) as u32,
            bytes.len() as u32,
        ];
        self.stream.record(spec::op::WRITE, &args, &[], None);
        self.held.push(bytes);
    }

    /// Commits a write; cancels an open not yet answered.
    pub fn close(&mut self, fd: Fd) {
        let args: [u32; CLOSE_ARGS] = [fd];
        self.stream.record(spec::op::CLOSE, &args, &[], None);
    }

    /// `op` carries the PROGRAM bit.
    pub fn call(&mut self, op: u32, args: &[u32], bytes: &[u8]) {
        xpute_kit::ensure!(op & spec::op::PROGRAM != 0, EINVAL, op);
        self.stream.record(op, args, &[], Some(bytes));
    }

    /// `call` run by the host before this returns, from what it already
    /// holds: its value or its errno, ENOSPC where the record does not fit
    /// past the turn's. What `args` name it writes during the call.
    pub fn call_now(&mut self, op: u32, args: &[u32], bytes: &[u8]) -> Result<u32, Errno> {
        xpute_kit::ensure!(op & spec::op::PROGRAM != 0, EINVAL, op);
        let at = self
            .stream
            .past(op, args, Some(bytes))
            .ok_or(Errno::ENOSPC)?;
        let res = crate::hostcall::hostcall(at as u32);
        u32::try_from(res).map_err(|_| Errno::of(res).unwrap_or(Errno::EPROTO))
    }

    /// `call` where it fits with `reserve` bytes left after it; false, and
    /// nothing recorded, where it does not, for the caller to ask again a later turn.
    pub fn try_call(&mut self, op: u32, args: &[u32], bytes: &[u8], reserve: usize) -> bool {
        if !self
            .stream
            .fits(record_bytes(args.len(), Some(bytes.len())), reserve)
        {
            return false;
        }
        self.call(op, args, bytes);
        true
    }

    /// Drops the turn's other calls and publishes the report alone, at once.
    pub fn abort(&mut self, errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
        self.stream.publish();
        let mut args: [u32; ABORT_ARGS] =
            [errnum as i32 as u32, at.line(), at.column(), 0, 0, 0, 0];
        for (i, v) in [(spec::abort::A, a), (spec::abort::B, b)] {
            args[i as usize] = v as u32;
            args[i as usize + 1] = (v >> 32) as u32;
        }
        self.stream
            .record(spec::op::ABORT, &args, &[], Some(at.file().as_bytes()));
        self.stream.publish();
    }
}
