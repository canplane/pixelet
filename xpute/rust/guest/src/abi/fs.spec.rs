// xpute-guest/abi/fs.spec.rs
//
// GENERATED from spec/abi/fs.json

//! The file device, on major FS: a guest's files, opened below a root the host bound, as
//! POSIX's `openat` and io_uring's submission and completion name them (ipc/fs.json).
//! The guest posts its calls (`import`), FS_OPENAT and FS_CLOSE with ACKREQ, tagged with
//! the descriptor; the host completes each of those on a later turn, a frame of the same
//! tag and command flagged RES on the submission ring, and does the rest in the gap.
//! xpute ships both ends: the guest's calls (xpute-guest ipc/door.rs, ipc/file.rs) and the
//! host's server (@xpute/host). A program binds only its roots to what backs them.

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no event or no enum read off the wire leaves a writer or a
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

/// A file's calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FsMinor {
    OPENAT = 0x02,
    READ = 0x03,
    WRITE = 0x04,
    CLOSE = 0x05,
}

impl FsMinor {
    pub const ALL: [FsMinor; 4] = [
        FsMinor::OPENAT,
        FsMinor::READ,
        FsMinor::WRITE,
        FsMinor::CLOSE,
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
    // ---- fs, import ----
    /// The file at `path` below `root` opened as `fd`, the frame's tag too. Completed with
    /// the file's size, or -errno: ENOENT where there is no such file, ECANCELED where FS_CLOSE
    /// came first, ENODEV where the machine has no file device.
    FS_OPENAT = 0x0302,
    /// The opened file's first `len` bytes copied to `at` in the guest's memory.
    FS_READ = 0x0303,
    /// The `len` bytes at `at` added to what the file opened for writing holds. The host
    /// reads them at FS_CLOSE, and the guest keeps them unchanged until FS_CLOSE is completed.
    FS_WRITE = 0x0304,
    /// The descriptor ended, the frame's tag too. A file opened for writing is replaced by
    /// what was written; an open not yet completed is completed first, with ECANCELED. Completed
    /// with 0, or -errno where the file could not be replaced.
    FS_CLOSE = 0x0305,
}

impl Command {
    pub const ALL: [Command; 4] = [
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

/// Command::FS_OPENAT's completion's packet.
#[derive(Clone, Copy, Debug)]
pub struct FsOpenatReply {
    /// Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on.
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

/// Command::FS_OPENAT's event.
pub struct FsOpenatEvent<'a> {
    pub fd: u32,
    pub root: u32,
    /// ipc/fs.json `flag`.
    pub flags: u32,
    pub path: &'a dyn Display,
}

impl FsOpenatEvent<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.root));
        w.u32(Some(self.flags));
        w.str_display(self.path);
    }
}

impl crate::ipc::door::Event for FsOpenatEvent<'_> {
    const NUMBER: u32 = 770;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        FsOpenatEvent::write(self, w);
    }
}

/// Command::FS_READ's event.
pub struct FsReadEvent {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl FsReadEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Event for FsReadEvent {
    const NUMBER: u32 = 771;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        FsReadEvent::write(self, w);
    }
}

/// Command::FS_WRITE's event.
pub struct FsWriteEvent {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl FsWriteEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Event for FsWriteEvent {
    const NUMBER: u32 = 772;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        FsWriteEvent::write(self, w);
    }
}

/// Command::FS_CLOSE's event.
pub struct FsCloseEvent {
    pub fd: u32,
}

impl FsCloseEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
    }
}

impl crate::ipc::door::Event for FsCloseEvent {
    const NUMBER: u32 = 773;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        FsCloseEvent::write(self, w);
    }
}
