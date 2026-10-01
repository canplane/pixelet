// xpute-runtime/ipc/file.rs

//! A guest's files: the descriptors it holds, each issued from one table with
//! whatever the program says it is for, over the system calls (ipc/sys.rs).
//!
//! The descriptor is the table's handle (abi/handle.rs), so the host's answer
//! to an open (SYS_OPENED) is looked up by it and reaches the descriptor's
//! owner; an answer to a descriptor already closed reaches no one, since its
//! generation is gone from the table and a descriptor issued since in the
//! same slot has another. Nothing is said of a closed descriptor after, so
//! this is the whole of a stale answer's handling.
//!
//! What an owner is, and where an answer goes once it is found, is the
//! program's: `O` is its own.

use crate::abi::handle::Slots;
use crate::ipc::sys::spec::flag;
use crate::ipc::sys::{Fd, Sys};

pub struct Files<O> {
    held: Slots<O>,
}

impl<O> Default for Files<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> Files<O> {
    pub const fn new() -> Self {
        Files { held: Slots::new() }
    }

    /// Opens `path` below `root` to read, for `owner`.
    pub fn open(&mut self, sys: &mut Sys, root: u32, path: &str, owner: O) -> Fd {
        let fd = self.held.insert(owner);
        sys.openat(fd, root, flag::O_RDONLY, path.as_bytes());
        fd
    }

    /// Ends `fd`, answered or not, and hands back its owner.
    pub fn close(&mut self, sys: &mut Sys, fd: Fd) -> Option<O> {
        sys.close(fd);
        self.held.remove(fd)
    }

    /// `bytes` replace the file at `path` below `root`: opened, written and
    /// closed in one turn, so no answer is waited for and none comes.
    pub fn write_whole(&mut self, sys: &mut Sys, root: u32, path: &str, bytes: Vec<u8>, owner: O) {
        let fd = self.held.insert(owner);
        sys.openat(fd, root, flag::O_WRONLY, path.as_bytes());
        sys.write(fd, bytes);
        self.close(sys, fd);
    }

    /// Whose an answer to an open is: none for a descriptor already closed.
    pub fn owner(&self, fd: Fd) -> Option<&O> {
        self.held.get(fd)
    }

    /// The descriptor held for an owner `which` picks, if one is.
    pub fn held_for(&self, which: impl Fn(&O) -> bool) -> Option<Fd> {
        self.held.iter().find(|(_, o)| which(o)).map(|(fd, _)| fd)
    }
}

#[cfg(test)]
#[path = "file.test.rs"]
mod test;
