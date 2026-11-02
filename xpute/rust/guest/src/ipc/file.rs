// xpute-guest/ipc/file.rs

//! A guest's open files, each with the owner `O` the program gives it. A
//! descriptor is a generational handle, so an answer to one already closed
//! reaches no one.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::cmd::SysOpened;
use crate::abi::handle::Table;
use crate::ipc::sys::spec::flag;
use crate::ipc::sys::{answer_of, Fd, Sys};

/// What an open came to, read once from the host's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    /// The file's size; the descriptor is its owner's to read and close.
    Bytes(u32),
    /// No such file; the descriptor is closed (`Files::complete_open`).
    Absent,
    /// Every other failure, and the number the host refused with; the
    /// descriptor is closed.
    Failed { errnum: Errno, detail: u32 },
}

impl Opened {
    pub fn of(opened: &SysOpened) -> Opened {
        match answer_of(opened.res) {
            Ok(n) => Opened::Bytes(n),
            Err(Errno::ENOENT) => Opened::Absent,
            Err(errnum) => Opened::Failed {
                errnum,
                detail: opened.detail,
            },
        }
    }
}

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
        self.try_open(sys, root, path, owner).or_bug(Errno::ENOSPC)
    }

    /// `open`, or none where `N - 1` are open already.
    pub fn try_open(&mut self, sys: &mut Sys, root: u32, path: &str, owner: O) -> Option<Fd> {
        let fd = self.held.insert(owner)?;
        sys.openat(fd, root, flag::O_RDONLY, path.as_bytes());
        Some(fd)
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
    pub fn complete_open(&mut self, sys: &mut Sys, opened: &SysOpened) -> Option<O> {
        let owner = self.owner(opened.fd).copied()?;
        if opened.res < 0 {
            self.close(sys, opened.fd);
        }
        Some(owner)
    }

    /// The file an open answered with `bytes`, read whole into the heap and
    /// closed: its buffer, filled from the next turn on, or ENOMEM where the
    /// heap has no room for it, nothing read.
    ///
    /// # Safety
    /// As `Sys::read`: the buffer is neither read, grown nor dropped before
    /// the next rise.
    pub unsafe fn read_whole(
        &mut self,
        sys: &mut Sys,
        fd: Fd,
        bytes: u32,
    ) -> Result<Vec<u8>, Errno> {
        let mut held = Vec::new();
        let room = held.try_reserve_exact(bytes as usize);
        if room.is_ok() {
            held.resize(bytes as usize, 0);
            // SAFETY: the caller's.
            unsafe { sys.read(fd, &mut held) };
        }
        self.close(sys, fd);
        room.map(|()| held).map_err(|_| Errno::ENOMEM)
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
