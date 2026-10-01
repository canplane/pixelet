// xpute-runtime/ipc/sys.rs

//! A guest's system calls (spec/xpute/ipc/sys.json): what it asks of its host
//! that no command answers, recorded in a stream the host runs once the turn
//! has returned. A call that names the guest's memory is done by the time the
//! next turn rises, and what it names is kept that long.
//!
//! Every program's calls are here — read a body the host holds, write a file,
//! report the invariant that stopped it. A program's own go in the same
//! stream through `call`, numbered past the PROGRAM bit.

use core::panic::Location;

use xpute_core::status::errno::Errno;

use crate::ipc::stream::Stream;
use crate::mem::base::off_of_slice;

#[path = "sys.spec.rs"]
pub mod spec;

pub struct Sys {
    stream: Stream,
    /// What a `write` names, kept until the host has run it.
    held: Vec<Vec<u8>>,
}

impl Sys {
    /// The calls over `words`, a range of the memory nothing else writes,
    /// which the directory names under SYS.
    pub fn new(words: &'static mut [u32]) -> Sys {
        Sys {
            stream: Stream::new(words, spec::HEAD as usize),
            held: Vec::new(),
        }
    }

    /// The turn's rise: what the host ran after the last turn is done, so what
    /// was kept for it goes.
    pub fn rise(&mut self) {
        self.held.clear();
    }

    /// The turn's fall: its calls, for the host to run once it returns.
    pub fn fall(&mut self) {
        self.stream.publish();
    }

    /// The bytes the host holds as `body` copied into `dst`, `dst`'s length of
    /// them, and let go. They are there from the next turn on, and `dst` must
    /// be kept until then.
    pub fn read(&mut self, body: u32, dst: &mut [u8]) {
        self.stream.record(spec::op::READ, &[body, off_of_slice(&raw mut *dst) as u32, dst.len() as u32], &[], None);
    }

    /// Lets the host's `body` go unread.
    pub fn close(&mut self, body: u32) {
        self.read(body, &mut []);
    }

    /// `bytes` written whole to the host's file `file`.
    pub fn write(&mut self, file: u32, bytes: Vec<u8>) {
        self.stream.record(spec::op::WRITE, &[file, off_of_slice(bytes.as_slice()) as u32, bytes.len() as u32], &[], None);
        self.held.push(bytes);
    }

    /// A program's own call: `op` with the PROGRAM bit, its arguments and its
    /// bytes.
    pub fn call(&mut self, op: u32, args: &[u32], bytes: &[u8]) {
        xpute_core::ensure!(op & spec::op::PROGRAM != 0, EINVAL, op);
        self.stream.record(op, args, &[], Some(bytes));
    }

    /// A broken invariant's report, as the one call of the turn and published
    /// at once, for the host to find when the stop reaches it: what was
    /// recorded before it is dropped, since none of it is to be done.
    pub fn abort(&mut self, errno: Errno, at: &Location<'_>, a: u64, b: u64) {
        self.stream.publish();
        let args = [errno as i32 as u32, at.line(), at.column(), a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        self.stream.record(spec::op::ABORT, &args, &[], Some(at.file().as_bytes()));
        self.stream.publish();
    }
}
