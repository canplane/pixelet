// xpute-guest/abi/cmd.spec.rs
//
// GENERATED from spec/abi/cmd.json

//! xpute's own protocol, the host's, on majors NOP, SYS and GPU: the guest's life as its host tells it.
//! NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME are the guest's
//! life as its host sees it: a host (re)attached, the guest about to be frozen, and woken;
//! what each means to keep or reset is the guest's. GPU_READY is the GPU grant's state:
//! the device the machine asked for answered. SYS_ABORT is the guest's end, the broken
//! invariant it reports as it stops, which the host hears as a VMM hears a guest's exit.
//! Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
//! children are its fields, in order; one with no fields carries no packet. Files are a
//! device of their own (fs.json). Majors 0x00 to 0x0F are xpute's (ipc/frame.json); a
//! program numbers its own protocols' commands past them.

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
    GPU = 0x02,
}

impl Major {
    pub const ALL: [Major; 3] = [Major::NOP, Major::SYS, Major::GPU];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The guest's life as its host tells it, and its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SysMinor {
    ABORT = 0x06,
    ATTACH = 0x07,
    SUSPEND = 0x08,
    RESUME = 0x09,
}

impl SysMinor {
    pub const ALL: [SysMinor; 4] = [
        SysMinor::ABORT,
        SysMinor::ATTACH,
        SysMinor::SUSPEND,
        SysMinor::RESUME,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SysMinor> {
        SysMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The GPU grant's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GpuMinor {
    READY = 0x01,
}

impl GpuMinor {
    pub const ALL: [GpuMinor; 1] = [GpuMinor::READY];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GpuMinor> {
        GpuMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- host, export ----
    /// Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    /// A host has attached, the first or again after one before it went: the mount sends it
    /// once the front is laid. What the old host's requests leave behind is the guest's to let go.
    SYS_ATTACH = 0x0107,
    /// The guest is about to be frozen, its turns stopped for as long as its host is hidden:
    /// its last sure moment to save. A call, rung at once.
    SYS_SUSPEND = 0x0108,
    /// The guest woken from SYS_SUSPEND. A call, rung at once.
    SYS_RESUME = 0x0109,
    /// The GPU the machine asked for answered, the first time or again after a loss, whose
    /// sign is in the stream's device block.
    GPU_READY = 0x0201,

    // ---- host, import ----
    /// The broken invariant the guest reports as it stops (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0106,
}

impl Command {
    pub const ALL: [Command; 6] = [
        Command::NOP,
        Command::SYS_ATTACH,
        Command::SYS_SUSPEND,
        Command::SYS_RESUME,
        Command::GPU_READY,
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
