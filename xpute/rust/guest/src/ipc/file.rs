// xpute-guest/ipc/file.rs

//! A guest's open files, each with the owner `O` the program gives it. A
//! descriptor is a generational handle, so an answer to one already closed
//! reaches no one; that is the whole of stale-answer handling.

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

    pub fn open(&mut self, sys: &mut Sys, root: u32, path: &str, owner: O) -> Fd {
        let fd = self.held.insert(owner);
        sys.openat(fd, root, flag::O_RDONLY, path.as_bytes());
        fd
    }

    pub fn close(&mut self, sys: &mut Sys, fd: Fd) -> Option<O> {
        sys.close(fd);
        self.held.remove(fd)
    }

    /// Opened, written and closed in one turn; no answer comes.
    pub fn write_whole(&mut self, sys: &mut Sys, root: u32, path: &str, bytes: Vec<u8>, owner: O) {
        let fd = self.held.insert(owner);
        sys.openat(fd, root, flag::O_WRONLY, path.as_bytes());
        sys.write(fd, bytes);
        self.close(sys, fd);
    }

    pub fn owner(&self, fd: Fd) -> Option<&O> {
        self.held.get(fd)
    }

    pub fn held_for(&self, which: impl Fn(&O) -> bool) -> Option<Fd> {
        self.held.iter().find(|(_, o)| which(o)).map(|(fd, _)| fd)
    }
}

#[cfg(test)]
#[path = "file.test.rs"]
mod test;
