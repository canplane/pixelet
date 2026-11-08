// xpute-core/abi/sys.spec.rs
//
// GENERATED from spec/abi/sys.json

//! xpute's own protocol on majors NOP and SYS. Through it the runtime tells the core about
//! the core's life and its turns, and the core reports its end. NOP is the rings' own test.
//! SYS_ATTACH, SYS_SUSPEND and SYS_RESUME tell the core that a runtime attached, for the
//! first time or again, that the core is about to be frozen, and that it was woken; what
//! each one means the core keeps or resets is the core's to decide. SYS_TURN marks a turn
//! that the runtime's scheduling policy grants, and carries the wall clock. SYS_ABORT
//! reports the broken invariant the core stops on, and the runtime hears it as a VMM hears
//! its guest exit.
//!
//! Each command is `major << 8 | minor` (kit abi/cmd). Its packet is an XTP branch whose
//! children are the command's fields, in order; a command with no fields carries no packet.
//! The GPU and the files are devices with protocols of their own (gpu.json, fs.json).
//! Majors 0x00 to 0x0F are xpute's (ipc/frame.json), and a program numbers the commands of
//! its own protocols past them.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SysMinor {
    ABORT = 0x01,
    ATTACH = 0x02,
    SUSPEND = 0x03,
    RESUME = 0x04,
    TURN = 0x05,
}

impl SysMinor {
    pub const ALL: [SysMinor; 5] = [
        SysMinor::ABORT,
        SysMinor::ATTACH,
        SysMinor::SUSPEND,
        SysMinor::RESUME,
        SysMinor::TURN,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SysMinor> {
        SysMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- sys, export ----
    /// The rings' own test. The reply carries the packet's value, or 0 when the frame has no
    /// packet.
    NOP = 0x0000,
    /// A runtime has attached, for the first time or after an earlier runtime went away. The
    /// runtime sends it once it has laid the front of the core's memory (ipc/head.json). The core
    /// lets go of whatever the earlier runtime's requests left behind.
    SYS_ATTACH = 0x0102,
    /// The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
    /// is the core's last sure moment to save. It is a call, and the runtime rings the core at
    /// once to have it answered.
    SYS_SUSPEND = 0x0103,
    /// The core has been woken after SYS_SUSPEND. It is a call, rung at once as SYS_SUSPEND is.
    SYS_RESUME = 0x0104,
    /// Marks a turn that the scheduling policy grants, as opposed to a turn rung only because a
    /// command arrived. The runtime submits it after what the program submits for that turn,
    /// and before it rings. It carries the wall clock in Unix milliseconds. Anything else the
    /// policy has to say about the turn, the program sends in commands of its own.
    SYS_TURN = 0x0105,

    // ---- sys, import ----
    /// Reports the broken invariant the core stops on (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0101,
}

impl Command {
    pub const ALL: [Command; 6] = [
        Command::NOP,
        Command::SYS_ATTACH,
        Command::SYS_SUSPEND,
        Command::SYS_RESUME,
        Command::SYS_TURN,
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

/// Command::SYS_TURN's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysTurn {
    pub unix_ms: f64,
}

impl SysTurn {
    pub fn read(pkt: &[u8]) -> Result<SysTurn, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysTurn {
            unix_ms: finite(f.at(0)?.get_number()?)?,
        })
    }
}

/// Command::SYS_ABORT's packet.
pub struct SysAbort<'a> {
    pub errnum: i32,
    pub line: u32,
    pub column: u32,
    pub a_low: u32,
    pub a_high: u32,
    pub b_low: u32,
    pub b_high: u32,
    pub file: &'a dyn Display,
}

impl SysAbort<'_> {
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

impl crate::ipc::door::Import for SysAbort<'_> {
    const NUMBER: u32 = 257;
    const FIELDS: u32 = 8;
    fn write(&self, w: &mut PacketWriter) {
        SysAbort::write(self, w);
    }
}
