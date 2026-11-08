// xpute-core/ipc/file.rs

//! A core's open files, each with the owner `O` the program gives it. A
//! descriptor is a generational handle, so a completion of one already
//! closed reaches no one.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::handle::Table;
use crate::call::answer_of;
use crate::ipc::door::{Door, Reading};
use crate::ipc::fs::spec::flag;
use crate::ipc::fs::{Fd, FsOpened};

/// What an open came to, read once from the runtime's completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    /// The file's size; the descriptor is its owner's to read and close.
    Bytes(u32),
    /// No such file; the descriptor is gone (`Files::complete_open`).
    Absent,
    /// Every other failure, and the number the runtime refused with; the
    /// descriptor is gone.
    Failed { errnum: Errno, detail: u32 },
}

impl Opened {
    pub fn of(opened: &FsOpened) -> Opened {
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

/// At most `N - 1` files are open at once; opening one more is a bug.
pub struct Files<O: Copy, const N: usize> {
    fds: Table<O, N>,
}

impl<O: Copy, const N: usize> Default for Files<O, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Copy, const N: usize> Files<O, N> {
    pub const fn new() -> Self {
        Files { fds: Table::new() }
    }

    pub fn open(&mut self, door: &Door, root: u32, path: &str, owner: O) -> Fd {
        let fd = self.fds.insert(owner).or_bug(Errno::ENOSPC);
        door.openat(fd, root, flag::O_RDONLY, path);
        fd
    }

    pub fn close(&mut self, door: &Door, fd: Fd) -> Option<O> {
        door.close(fd);
        self.fds.remove(fd)
    }

    /// Opens, writes and closes a file in one turn. Its completions reach no
    /// owner.
    pub fn write_whole(&mut self, door: &Door, root: u32, path: &str, bytes: Vec<u8>, owner: O) {
        let fd = self.fds.insert(owner).or_bug(Errno::ENOSPC);
        door.openat(fd, root, flag::O_WRONLY, path);
        door.write(fd, bytes);
        self.close(door, fd);
    }

    /// The owner of the open that `opened` completes, or none when its
    /// descriptor has been closed since. A failed open leaves no descriptor
    /// to close, so its handle is let go here.
    pub fn complete_open(&mut self, opened: &FsOpened) -> Option<O> {
        let owner = self.owner(opened.fd).copied()?;
        if opened.res < 0 {
            self.fds.remove(opened.fd);
        }
        Some(owner)
    }

    /// Reads the whole file, whose open completed with `bytes`, into the heap,
    /// and closes it. Answers the reading, whose bytes `Reading::take` hands
    /// over once the runtime's head has passed it, or ENOMEM when the heap has
    /// no room for them and nothing was read.
    pub fn read_whole(&mut self, door: &Door, fd: Fd, bytes: u32) -> Result<Reading, Errno> {
        let mut buf = Vec::new();
        let room = buf.try_reserve_exact(bytes as usize);
        let read = room.map(|()| {
            buf.resize(bytes as usize, 0);
            door.read_buf(fd, buf)
        });
        self.close(door, fd);
        read.map_err(|_| Errno::ENOMEM)
    }

    pub fn owner(&self, fd: Fd) -> Option<&O> {
        self.fds.get(fd)
    }

    pub fn find(&self, which: impl Fn(&O) -> bool) -> Option<Fd> {
        self.fds.iter().find(|(_, o)| which(o)).map(|(fd, _)| fd)
    }
}

#[cfg(test)]
#[path = "file.test.rs"]
mod test;
