// xpute-runtime/ipc/sys.rs

//! A guest's system calls, recorded in a stream the host runs after the turn
//! returns; anything a call names must live until the next rise. The guest
//! issues its own descriptors as handles, so a late answer to a closed one
//! names nothing. `openat` is answered by SYS_OPENED on a later turn.

use core::panic::Location;

use xpute_core::status::errno::Errno;

use crate::ipc::stream::Stream;
use crate::mem::base::off_of_slice;

#[path = "sys.spec.rs"]
pub mod spec;

pub type Fd = u32;

pub struct Sys {
    stream: Stream,
    /// Kept until the host has run the `write` that names it.
    held: Vec<Vec<u8>>,
}

impl Sys {
    /// `words` is the range the directory names under SYS.
    pub fn new(words: &'static mut [u32]) -> Sys {
        Sys {
            stream: Stream::new(words, spec::HEAD as usize),
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
        self.stream.record(spec::op::OPENAT, &[fd, root, flags], &[], Some(path));
    }

    /// `dst` is filled by the next turn and must live until then.
    pub fn read(&mut self, fd: Fd, dst: &mut [u8]) {
        self.stream.record(spec::op::READ, &[fd, off_of_slice(&raw mut *dst) as u32, dst.len() as u32], &[], None);
    }

    pub fn write(&mut self, fd: Fd, bytes: Vec<u8>) {
        self.stream.record(spec::op::WRITE, &[fd, off_of_slice(bytes.as_slice()) as u32, bytes.len() as u32], &[], None);
        self.held.push(bytes);
    }

    /// Commits a write; cancels an open not yet answered.
    pub fn close(&mut self, fd: Fd) {
        self.stream.record(spec::op::CLOSE, &[fd], &[], None);
    }

    /// `op` carries the PROGRAM bit.
    pub fn call(&mut self, op: u32, args: &[u32], bytes: &[u8]) {
        xpute_core::ensure!(op & spec::op::PROGRAM != 0, EINVAL, op);
        self.stream.record(op, args, &[], Some(bytes));
    }

    /// Drops the turn's other calls and publishes the report alone, at once.
    pub fn abort(&mut self, errno: Errno, at: &Location<'_>, a: u64, b: u64) {
        self.stream.publish();
        let args = [errno as i32 as u32, at.line(), at.column(), a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        self.stream.record(spec::op::ABORT, &args, &[], Some(at.file().as_bytes()));
        self.stream.publish();
    }
}
