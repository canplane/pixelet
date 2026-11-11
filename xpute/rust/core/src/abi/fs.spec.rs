// xpute-core/abi/fs.spec.rs
//
// GENERATED from spec/abi/fs.json

//! The file device's protocol on major FS: a core's files, read whole into its own buffers
//! and written whole, through a descriptor, as WASI's path_open, fd_read, fd_write and
//! fd_close do. A file's name is an absolute path, a tuple of tokens, each an i32
//! or a string, as many as the program uses. xpute gives no token a meaning, and the
//! program's one binding answers every path (@xpute/runtime io/fs.ts).
//!
//! The core sends its calls as imports. The core issues each descriptor, a generational
//! handle, and every frame of a descriptor carries it once, as its tag. FS_OPEN and FS_CLOSE
//! go with ACKREQ, and the runtime completes each once, on a later turn, with a frame of the
//! same tag and command, flagged RES. It serves a descriptor's frames in the order they came,
//! so an FS_READ is done before its close is completed, and the memory an FS_READ names is the
//! runtime's until then. An FS_WRITE's bytes go with its frame, and the file is replaced by
//! them at FS_CLOSE. The runtime tells the core that a file changed under it with
//! FS_CHANGED, an export. xpute ships both ends: the core's calls (xpute-core io/door.rs,
//! io/file.rs) and the runtime's server (@xpute/runtime). A program only gives the binding
//! that backs its paths.

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no import or no enum read off the wire leaves a writer or a
// number's check unused.
#![allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    clippy::absolute_paths,
    unused_imports,
    dead_code,
    reason = "generated from the spec"
)]

use core::fmt::Display;

use crate::abi::number::{finite, integer, unknown};
use crate::io::fs::{Path, Token};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// A command's major, the high byte of its number, by which the runtime routes the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    FS = 0x03,
}

impl Major {
    pub const ALL: [Major; 1] = [Major::FS];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FsMinor {
    OPEN = 0x01,
    READ = 0x02,
    WRITE = 0x03,
    CLOSE = 0x04,
    CHANGED = 0x05,
}

impl FsMinor {
    pub const ALL: [FsMinor; 5] = [
        FsMinor::OPEN,
        FsMinor::READ,
        FsMinor::WRITE,
        FsMinor::CLOSE,
        FsMinor::CHANGED,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<FsMinor> {
        FsMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- fs, export ----
    /// The file at `path` changed under the core, as reported by what backs it. What the core
    /// has read of the file is stale, and to read it again the core opens it again.
    FS_CHANGED = 0x0305,

    // ---- fs, import ----
    /// Opens the file at `path` as the descriptor the frame's tag carries, as WASI's path_open
    /// does. The runtime completes an open to read once the file can be read whole, with its size,
    /// and an open to write with 0; or either with -errno: ENOENT when there is no such file to
    /// read, EINVAL when `flags` is not one of io/fs.json's `flag`, which the descriptor's close
    /// then completes with too, ECANCELED when FS_CLOSE came first, and ENODEV when the manifest has
    /// no file device.
    FS_OPEN = 0x0301,
    /// Copies the first `len` bytes of the opened file to `at` in the core's memory, as WASI's
    /// fd_read reads into a caller's buffer. A read of a file opened to write, or of one whose
    /// open has not completed, copies nothing, and the descriptor's close completes with EBADF.
    /// Where the manifest has no file device, it is refused with ENODEV.
    FS_READ = 0x0302,
    /// Appends the `len` bytes at `at` to what the file, opened to write, will hold, as WASI's
    /// fd_write does. The bytes go with the frame, so the core keeps nothing. Nothing completes
    /// it: the file's close says whether what was written replaced it. A write to a file opened
    /// to read keeps nothing, and the descriptor's close completes with EBADF.
    /// Where the manifest has no file device, it is refused with ENODEV.
    FS_WRITE = 0x0303,
    /// Ends the descriptor, as WASI's fd_close does. A file opened to write is replaced by what
    /// was written to it, handed to the program's binding whole. An open not yet completed is
    /// completed first, with ECANCELED. The close is completed with: 0; the errno the binding
    /// refused the written file with; EINVAL where its open was refused for its flags; EBADF for a
    /// descriptor the runtime does not hold, or one read while opened to write or before its open
    /// completed, or written while opened to read; EFAULT where a read of it named memory outside
    /// the core's or at offset 0; ENODEV where the manifest has no file device; or ECANCELED where a
    /// reload of the program left the file unserved.
    FS_CLOSE = 0x0304,
}

impl Command {
    pub const ALL: [Command; 5] = [
        Command::FS_CHANGED,
        Command::FS_OPEN,
        Command::FS_READ,
        Command::FS_WRITE,
        Command::FS_CLOSE,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::FS_CHANGED's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsChanged<'a> {
    /// The file's path, as the core opened it.
    pub path: Path<'a>,
}

impl<'a> FsChanged<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<FsChanged<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsChanged {
            path: Path::read(pkt, f.at(0)?)?,
        })
    }
}

/// Command::FS_OPEN's completion's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsOpenReply {
    /// The number the runtime refused the open with, such as an HTTP status. It is kept to be
    /// printed, and nothing should branch on it.
    pub detail: u32,
}

impl FsOpenReply {
    pub fn read(pkt: &[u8]) -> Result<FsOpenReply, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsOpenReply {
            detail: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::FS_OPEN's packet.
pub struct FsOpen<'a> {
    pub path: &'a [Token<'a>],
    /// One of the `flag` values of io/fs.json.
    pub flags: u32,
}

impl FsOpen<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        Path::write(w, self.path);
        w.u32(Some(self.flags));
    }
}

impl crate::io::door::Import for FsOpen<'_> {
    const NUMBER: u32 = 769;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = true;
    fn write(&self, w: &mut PacketWriter) {
        FsOpen::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// Command::FS_READ's packet.
pub struct FsRead {
    pub at: u32,
    pub len: u32,
}

impl FsRead {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::io::door::Import for FsRead {
    const NUMBER: u32 = 770;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        FsRead::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// Command::FS_WRITE's packet.
pub struct FsWrite {
    pub at: u32,
    pub len: u32,
}

impl FsWrite {
    /// The children it is written as.
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::io::door::Import for FsWrite {
    const NUMBER: u32 = 771;
    const FIELDS: u32 = 2;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        FsWrite::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (self.at, self.len)
    }
}

/// Command::FS_CLOSE's packet.
pub struct FsClose {}

impl FsClose {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl crate::io::door::Import for FsClose {
    const NUMBER: u32 = 772;
    const FIELDS: u32 = 0;
    const ACKREQ: bool = true;
    fn write(&self, w: &mut PacketWriter) {
        FsClose::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

use crate::host::answer_of;
/// The fs protocol's core end: what it posts to the runtime and what it hears from it.
use crate::io::door::{door, Completion as Done, Outcome};
use xpute_kit::status::errno::Errno;

/// Command::FS_OPEN, completed on a later turn (`Completion::FsOpen`).
pub fn fs_open(fd: u32, e: &FsOpen<'_>) {
    door().import(fd, e);
}

/// Command::FS_READ, posted for the runtime.
pub fn fs_read(fd: u32, e: &FsRead) {
    door().import(fd, e);
}

/// Command::FS_WRITE, posted for the runtime.
pub fn fs_write(fd: u32, e: &FsWrite) {
    door().import(fd, e);
}

/// Command::FS_CLOSE, completed on a later turn (`Completion::FsClose`).
pub fn fs_close(fd: u32) {
    door().import(fd, &FsClose {});
}

/// What the program does with each command the runtime sends.
pub trait Exports {
    fn fs_changed(&mut self, c: FsChanged<'_>) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn apply<H: Exports>(h: &mut H, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
    match cmd {
        x if x == Command::FS_CHANGED as u32 => h.fs_changed(FsChanged::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}

/// A request's completion, under the handle it was made with, and its result.
#[derive(Clone, Copy, Debug)]
pub enum Completion {
    FsOpen {
        fd: u32,
        res: Result<u32, Errno>,
        reply: Option<FsOpenReply>,
    },
    FsClose {
        fd: u32,
        res: Result<u32, Errno>,
    },
}

/// `c` read as one of this protocol's completions, or none for another's.
pub fn completion(c: &Done<'_>) -> Option<Completion> {
    let res = answer_of(c.res);
    match c.cmd {
        x if x == Command::FS_OPEN as u32 => Some(Completion::FsOpen {
            fd: c.tag,
            res,
            reply: c.pkt.and_then(|pkt| FsOpenReply::read(pkt).ok()),
        }),
        x if x == Command::FS_CLOSE as u32 => Some(Completion::FsClose { fd: c.tag, res }),
        _ => None,
    }
}
