// xpute-core/abi/fs.spec.rs
//
// GENERATED from spec/abi/fs.json

//! The file device's protocol on major FS: a core's files, each opened below a root the
//! runtime has bound, in the terms of POSIX's `openat` and of io_uring's submissions and
//! completions (ipc/fs.json). The core posts its calls as imports. It posts FS_OPENAT and
//! FS_CLOSE with ACKREQ, tagged with the descriptor, and the runtime completes each of them
//! on a later turn with a frame of the same tag and command, flagged RES, on the submission
//! ring. The runtime does the rest of the work in the gap between turns. It tells the core
//! that a file changed under it with FS_CHANGED, an export. xpute ships both ends: the core's
//! calls (xpute-core ipc/door.rs, ipc/file.rs) and the runtime's server (@xpute/runtime). A
//! program only binds its roots to what backs them.

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
    OPENAT = 0x01,
    READ = 0x02,
    WRITE = 0x03,
    CLOSE = 0x04,
    CHANGED = 0x05,
}

impl FsMinor {
    pub const ALL: [FsMinor; 5] = [
        FsMinor::OPENAT,
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
    /// The file at `path` below `root` changed under the core, as reported by what backs the
    /// root. What the core has read of the file is stale, and to read it again the core opens
    /// it again.
    FS_CHANGED = 0x0305,

    // ---- fs, import ----
    /// Opens the file at `path` below `root` as descriptor `fd`, which is also the frame's tag.
    /// The runtime completes it with the file's size, or with -errno: ENOENT when there is no
    /// such file, ECANCELED when FS_CLOSE came first, and ENODEV when the machine has no file
    /// device.
    FS_OPENAT = 0x0301,
    /// Copies the first `len` bytes of the opened file to `at` in the core's memory.
    FS_READ = 0x0302,
    /// Appends the `len` bytes at `at` to what the file, opened for writing, will hold. The
    /// runtime reads the bytes at FS_CLOSE, so the core keeps them unchanged until FS_CLOSE is
    /// completed.
    FS_WRITE = 0x0303,
    /// Ends the descriptor, which is also the frame's tag. A file opened for writing is replaced
    /// by what was written to it. An open not yet completed is completed first, with ECANCELED.
    /// The close is completed with 0, or with -errno when the file could not be replaced.
    FS_CLOSE = 0x0304,
}

impl Command {
    pub const ALL: [Command; 5] = [
        Command::FS_CHANGED,
        Command::FS_OPENAT,
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
    pub root: u32,
    /// The file's path below the root, spelled as the core opened it.
    pub path: &'a str,
}

impl<'a> FsChanged<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<FsChanged<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsChanged {
            root: integer::<u32>(f.at(0)?.get_number()?)?,
            path: f.at(1)?.get_str_ref()?,
        })
    }
}

/// Command::FS_OPENAT's completion's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsOpenatReply {
    /// The number the runtime refused the open with, such as an HTTP status. It is kept to be
    /// printed, and nothing should branch on it.
    pub detail: u32,
}

impl FsOpenatReply {
    pub fn read(pkt: &[u8]) -> Result<FsOpenatReply, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(FsOpenatReply {
            detail: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::FS_OPENAT's packet.
pub struct FsOpenat<'a> {
    pub fd: u32,
    pub root: u32,
    /// One of the `flag` values of ipc/fs.json.
    pub flags: u32,
    pub path: &'a dyn Display,
}

impl FsOpenat<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.root));
        w.u32(Some(self.flags));
        w.str_display(self.path);
    }
}

impl crate::ipc::door::Import for FsOpenat<'_> {
    const NUMBER: u32 = 769;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        FsOpenat::write(self, w);
    }
}

/// Command::FS_READ's packet.
pub struct FsRead {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl FsRead {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Import for FsRead {
    const NUMBER: u32 = 770;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        FsRead::write(self, w);
    }
}

/// Command::FS_WRITE's packet.
pub struct FsWrite {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl FsWrite {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Import for FsWrite {
    const NUMBER: u32 = 771;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        FsWrite::write(self, w);
    }
}

/// Command::FS_CLOSE's packet.
pub struct FsClose {
    pub fd: u32,
}

impl FsClose {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
    }
}

impl crate::ipc::door::Import for FsClose {
    const NUMBER: u32 = 772;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        FsClose::write(self, w);
    }
}
