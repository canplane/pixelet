// xpute-core/io/file.rs

//! `FileTable` holds a core's open files and the requests it awaits, two
//! things apart. An open file is a descriptor, a generational handle, with the
//! owner `O` the program gives it. A descriptor is let go as its close is
//! sent, as POSIX `close()` lets one go, and what then awaits the close's
//! completion is a request `R`, found by its tag, the descriptor with its
//! generation, as io_uring finds a completion by its `user_data`: a slot taken
//! again issues another tag, so no old completion reaches its new holder. A
//! read's request is held with the buffer it lent the runtime (`StableBuf`),
//! which the table owns until the close is completed. The file calls are fs's
//! generated functions, over the core's door, and each completion, which fs's
//! generated `completion` reads, hands back what it is owed: an open's to its
//! owner with what the open came to (`complete_open`), a close's to its request
//! with the buffer, filled, and the close's result (`complete_close`).

use core::ops::DerefMut;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::fs::{fs_close, fs_open, fs_read, fs_write, FsOpen, FsRead, FsWrite};
use crate::abi::handle::Table;
use crate::io::door::door;
use crate::io::fs::spec::flag;
use crate::io::fs::{Fd, Token};

/// A buffer a read fills, whose bytes stay where they are while the buffer
/// moves, as a heap allocation's do.
///
/// # Safety
/// Moving the buffer moves none of the bytes it derefs to.
pub unsafe trait StableBuf: DerefMut<Target = [u8]> {}

// SAFETY: a Vec's bytes are on the heap, and moving the Vec moves its handle
// alone.
unsafe impl StableBuf for Vec<u8> {}

/// `OpenResult` is what an open to read came to, read once from the runtime's
/// completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenResult {
    /// The file's size; the descriptor is its owner's to read or close.
    Bytes(u32),
    /// No such file; the descriptor is gone.
    Absent,
    /// Every other failure, and the number the runtime refused with; the
    /// descriptor is gone.
    Failed { errnum: Errno, detail: u32 },
}

impl OpenResult {
    /// What an open completed with `res` came to, and `detail`, the number
    /// the runtime refused it with.
    pub fn of(res: Result<u32, Errno>, detail: u32) -> OpenResult {
        match res {
            Ok(n) => OpenResult::Bytes(n),
            Err(Errno::ENOENT) => OpenResult::Absent,
            Err(errnum) => OpenResult::Failed { errnum, detail },
        }
    }
}

/// At most `N - 1` files are open at once; opening one more is a bug. A
/// written file takes no slot.
pub struct FileTable<O: Copy, R, B: StableBuf, const N: usize> {
    fds: Table<O, N>,
    /// Each request whose close is not yet completed, by its tag, with the
    /// buffer a read lent the runtime.
    requests: Vec<(Fd, R, Option<B>)>,
}

impl<O: Copy, R, B: StableBuf, const N: usize> Default for FileTable<O, R, B, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Copy, R, B: StableBuf, const N: usize> FileTable<O, R, B, N> {
    pub const fn new() -> Self {
        FileTable {
            fds: Table::new(),
            requests: Vec::new(),
        }
    }

    /// Opens the file at `path` to be read whole.
    pub fn open(&mut self, path: &[Token], owner: O) -> Fd {
        let fd = self.fds.insert(owner).or_bug(Errno::EMFILE);
        let flags = flag::O_RDONLY;
        fs_open(fd, &FsOpen { path, flags });
        fd
    }

    /// Replaces the file at `path` with `bytes`: opens it to write, writes and
    /// closes it in one turn, the bytes going with their frame. The descriptor
    /// is let go as its close is sent, and `request` hears the close's result
    /// (`complete_close`). A tag still awaited, as many writes on as a
    /// generation counts, is a bug.
    pub fn write_whole(&mut self, path: &[Token], bytes: &[u8], request: R) -> Fd {
        let fd = door().tag();
        let awaited = self.requests.iter().any(|(t, ..)| *t == fd);
        xpute_kit::ensure!(!awaited, EMFILE, fd);
        let flags = flag::O_WRONLY;
        fs_open(fd, &FsOpen { path, flags });
        let at = door().mem().off_of_slice(bytes) as u32;
        let len = bytes.len() as u32;
        fs_write(fd, &FsWrite { at, len });
        fs_close(fd);
        self.requests.push((fd, request, None));
        fd
    }

    /// Reads the open file `fd` whole into `buf` and closes it. The descriptor
    /// is let go as its close is sent, and `request` is held with the buffer
    /// until the close is completed, which hands both back, the buffer filled.
    /// A descriptor not open is let be, and `buf` dropped.
    pub fn read_whole(&mut self, fd: Fd, mut buf: B, request: R) {
        if self.fds.remove(fd).is_none() {
            return;
        }
        // The runtime fills the buffer before the close of `fd` completes, and
        // the table holds it until then, moving none of its bytes
        // (`StableBuf`).
        let at = door().mem().off_of_slice(&raw mut *buf) as u32;
        let len = buf.len() as u32;
        fs_read(fd, &FsRead { at, len });
        fs_close(fd);
        self.requests.push((fd, request, Some(buf)));
    }

    /// Closes the open file `fd`, whose owner hears nothing more of it; an
    /// open not yet completed is completed with ECANCELED. Answers EBADF for
    /// a descriptor not open.
    pub fn close(&mut self, fd: Fd) -> Result<(), Errno> {
        self.fds.remove(fd).ok_or(Errno::EBADF)?;
        fs_close(fd);
        Ok(())
    }

    /// Hands the owner of the open of `fd`, completed with `res` and the
    /// runtime's `detail`, what it came to, or none where the descriptor is no
    /// longer open. A failed open leaves no descriptor to close, so its handle
    /// is let go here.
    pub fn complete_open(
        &mut self,
        fd: Fd,
        res: Result<u32, Errno>,
        detail: u32,
    ) -> Option<(O, OpenResult)> {
        let owner = *self.fds.get(fd)?;
        if res.is_err() {
            self.fds.remove(fd);
        }
        Some((owner, OpenResult::of(res, detail)))
    }

    /// Hands back the request whose close of `fd` completed with `res`, with
    /// a read's buffer, or none for a close no request awaits.
    pub fn complete_close(
        &mut self,
        fd: Fd,
        res: Result<u32, Errno>,
    ) -> Option<(R, Option<B>, Result<(), Errno>)> {
        let at = self.requests.iter().position(|(t, ..)| *t == fd)?;
        let (_, request, buf) = self.requests.swap_remove(at);
        Some((request, buf, res.map(|_| ())))
    }

    /// The open file whose owner `which` picks.
    pub fn find(&self, which: impl Fn(&O) -> bool) -> Option<Fd> {
        self.fds.iter().find(|(_, o)| which(o)).map(|(fd, _)| fd)
    }
}

#[cfg(test)]
#[path = "file.test.rs"]
mod test;
