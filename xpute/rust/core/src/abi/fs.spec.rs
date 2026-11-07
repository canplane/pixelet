// xpute-core/abi/fs.spec.rs
//
// GENERATED from spec/abi/fs.json

//! The file device, on major FS: a core's files, opened below a root the runtime bound, as
//! POSIX's `openat` and io_uring's submission and completion name them (ipc/fs.json).
//! The core posts its calls (`import`), FS_OPENAT and FS_CLOSE with ACKREQ, tagged with
//! the descriptor; the runtime completes each of those on a later turn, a frame of the same
//! tag and command flagged RES on the submission ring, and does the rest in the gap.
//! xpute ships both ends: the core's calls (xpute-core ipc/door.rs, ipc/file.rs) and the
//! runtime's server (@xpute/runtime). A program binds only its roots to what backs them.
//! The runtime tells the core a file changed under it (FS_CHANGED, `export`).

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

/// What a command is about.
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

/// A file's calls, and a file changed.
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
    /// The file at `path` below `root` changed under the core, as what backs it says: what the
    /// core read of it is stale, and a read opens it again.
    FS_CHANGED = 0x0305,

    // ---- fs, import ----
    /// The file at `path` below `root` opened as `fd`, the frame's tag too. Completed with
    /// the file's size, or -errno: ENOENT where there is no such file, ECANCELED where FS_CLOSE
    /// came first, ENODEV where the machine has no file device.
    FS_OPENAT = 0x0301,
    /// The opened file's first `len` bytes copied to `at` in the core's memory.
    FS_READ = 0x0302,
    /// The `len` bytes at `at` added to what the file opened for writing holds. The runtime
    /// reads them at FS_CLOSE, and the core keeps them unchanged until FS_CLOSE is completed.
    FS_WRITE = 0x0303,
    /// The descriptor ended, the frame's tag too. A file opened for writing is replaced by
    /// what was written; an open not yet completed is completed first, with ECANCELED. Completed
    /// with 0, or -errno where the file could not be replaced.
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
    /// Its path below the root, as the core opened it.
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
    /// Whatever number the runtime refused with — for HTTP its status — kept for a reader to print and for nothing to branch on.
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
    /// ipc/fs.json `flag`.
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
