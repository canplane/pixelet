// xpute-guest/ipc/file.rs

//! A guest's open files, each with the owner `O` the program gives it. A
//! descriptor is a generational handle, so an answer to one already closed
//! reaches no one; that is the whole of stale-answer handling.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::cmd::SysOpened;
use crate::abi::handle::Table;
use crate::ipc::sys::spec::flag;
use crate::ipc::sys::{Fd, Sys};

/// At most `N - 1` open at once; one more is a bug.
pub struct Files<O: Copy, const N: usize> {
    held: Table<O, N>,
}

impl<O: Copy, const N: usize> Default for Files<O, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Copy, const N: usize> Files<O, N> {
    pub const fn new() -> Self {
        Files { held: Table::new() }
    }

    pub fn open(&mut self, sys: &mut Sys, root: u32, path: &str, owner: O) -> Fd {
        let fd = self.held.insert(owner).or_bug(Errno::ENOSPC);
        sys.openat(fd, root, flag::O_RDONLY, path.as_bytes());
        fd
    }

    pub fn close(&mut self, sys: &mut Sys, fd: Fd) -> Option<O> {
        sys.close(fd);
        self.held.remove(fd)
    }

    /// Opened, written and closed in one turn; no answer comes.
    pub fn write_whole(&mut self, sys: &mut Sys, root: u32, path: &str, bytes: Vec<u8>, owner: O) {
        let fd = self.held.insert(owner).or_bug(Errno::ENOSPC);
        sys.openat(fd, root, flag::O_WRONLY, path.as_bytes());
        sys.write(fd, bytes);
        self.close(sys, fd);
    }

    /// The owner of the open `opened` completes, or none where its descriptor
    /// was closed since; a descriptor whose open failed is closed here.
    pub fn opened(&mut self, sys: &mut Sys, opened: &SysOpened) -> Option<O> {
        let owner = self.owner(opened.fd).copied()?;
        if opened.res < 0 {
            self.close(sys, opened.fd);
        }
        Some(owner)
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
