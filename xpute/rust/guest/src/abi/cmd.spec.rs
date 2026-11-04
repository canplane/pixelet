// xpute-guest/abi/cmd.spec.rs
//
// GENERATED from spec/abi/cmd.json

//! xpute's own commands, by number (kit abi/cmd: `major << 8 | minor`), and the packet
//! each carries — an XTP branch whose children are its fields, in order. A command with
//! no fields carries no packet. The host sends them, port 0 (ipc/frame.json HOST); a
//! program numbers its own as it likes, and the sender tells the two apart.
//!
//! The direction is the ring: `pkt` is what the host submits, `event` what the guest
//! posts on the completion ring for the host's port, its hostcalls (ipc/sys.json).

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
    NOP = 0x00,
    SYS = 0x01,
}

impl Major {
    pub const ALL: [Major; 2] = [Major::NOP, Major::SYS];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The hostcalls and their answers (ipc/sys.json).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SysMinor {
    OPENED = 0x01,
    OPENAT = 0x02,
    READ = 0x03,
    WRITE = 0x04,
    CLOSE = 0x05,
    ABORT = 0x06,
}

impl SysMinor {
    pub const ALL: [SysMinor; 6] = [
        SysMinor::OPENED,
        SysMinor::OPENAT,
        SysMinor::READ,
        SysMinor::WRITE,
        SysMinor::CLOSE,
        SysMinor::ABORT,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SysMinor> {
        SysMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// xpute's own commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    /// Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    /// An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
    /// Never sent for a descriptor already closed.
    SYS_OPENED = 0x0101,
    /// The file at `path` below `root` opened as `fd`, answered by SYS_OPENED.
    SYS_OPENAT = 0x0102,
    /// The opened file's first `len` bytes copied to `at` in the guest's memory.
    SYS_READ = 0x0103,
    /// The `len` bytes at `at` added to what the file opened for writing holds.
    SYS_WRITE = 0x0104,
    /// The descriptor ended. A file opened for writing is replaced by what was written; an
    /// open not yet answered is canceled, and never answered.
    SYS_CLOSE = 0x0105,
    /// The broken invariant the guest reports as it stops (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0106,
}

impl Command {
    pub const ALL: [Command; 7] = [
        Command::NOP,
        Command::SYS_OPENED,
        Command::SYS_OPENAT,
        Command::SYS_READ,
        Command::SYS_WRITE,
        Command::SYS_CLOSE,
        Command::SYS_ABORT,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::NOP's packet.
#[derive(Clone, Copy, Debug)]
pub struct Nop {
    pub val: u32,
}

impl Nop {
    pub fn read(pkt: &[u8]) -> Result<Nop, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(Nop {
            val: integer::<u32>(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SYS_OPENED's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysOpened {
    pub fd: u32,
    pub res: i32,
    /// Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on.
    pub detail: u32,
}

impl SysOpened {
    pub fn read(pkt: &[u8]) -> Result<SysOpened, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysOpened {
            fd: integer::<u32>(f.at(0)?.get_number()?)?,
            res: integer::<i32>(f.at(1)?.get_number()?)?,
            detail: integer::<u32>(f.at(2)?.get_number()?)?,
        })
    }
}

/// Command::SYS_OPENAT's event.
pub struct SysOpenatEvent<'a> {
    pub fd: u32,
    pub root: u32,
    /// ipc/sys.json `flag`.
    pub flags: u32,
    pub path: &'a dyn Display,
}

impl SysOpenatEvent<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 4;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.root));
        w.u32(Some(self.flags));
        w.str_display(self.path);
    }
}

impl crate::ipc::door::Event for SysOpenatEvent<'_> {
    const NUMBER: u32 = 258;
    const FIELDS: u32 = 4;
    fn write(&self, w: &mut PacketWriter) {
        SysOpenatEvent::write(self, w);
    }
}

/// Command::SYS_READ's event.
pub struct SysReadEvent {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl SysReadEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Event for SysReadEvent {
    const NUMBER: u32 = 259;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        SysReadEvent::write(self, w);
    }
}

/// Command::SYS_WRITE's event.
pub struct SysWriteEvent {
    pub fd: u32,
    pub at: u32,
    pub len: u32,
}

impl SysWriteEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
        w.u32(Some(self.at));
        w.u32(Some(self.len));
    }
}

impl crate::ipc::door::Event for SysWriteEvent {
    const NUMBER: u32 = 260;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        SysWriteEvent::write(self, w);
    }
}

/// Command::SYS_CLOSE's event.
pub struct SysCloseEvent {
    pub fd: u32,
}

impl SysCloseEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.fd));
    }
}

impl crate::ipc::door::Event for SysCloseEvent {
    const NUMBER: u32 = 261;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SysCloseEvent::write(self, w);
    }
}

/// Command::SYS_ABORT's event.
pub struct SysAbortEvent<'a> {
    pub errnum: i32,
    pub line: u32,
    pub column: u32,
    pub a_low: u32,
    pub a_high: u32,
    pub b_low: u32,
    pub b_high: u32,
    pub file: &'a dyn Display,
}

impl SysAbortEvent<'_> {
    /// The children it is written as.
    pub const FIELDS: u32 = 8;

    pub fn write(&self, w: &mut PacketWriter) {
        w.i32(Some(self.errnum));
        w.u32(Some(self.line));
        w.u32(Some(self.column));
        w.u32(Some(self.a_low));
        w.u32(Some(self.a_high));
        w.u32(Some(self.b_low));
        w.u32(Some(self.b_high));
        w.str_display(self.file);
    }
}

impl crate::ipc::door::Event for SysAbortEvent<'_> {
    const NUMBER: u32 = 262;
    const FIELDS: u32 = 8;
    fn write(&self, w: &mut PacketWriter) {
        SysAbortEvent::write(self, w);
    }
}
