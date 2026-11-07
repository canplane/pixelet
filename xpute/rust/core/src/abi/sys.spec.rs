// xpute-core/abi/sys.spec.rs
//
// GENERATED from spec/abi/sys.json

//! xpute's own protocol on majors NOP and SYS: the core's life as its runtime tells it, its
//! turns, and its end. NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME
//! are the core's life as its runtime sees it: a runtime (re)attached, the core about to be
//! frozen, and woken; what each means to keep or reset is the core's. SYS_TURN marks a turn
//! the runtime's scheduling policy grants, with the wall clock. SYS_ABORT is the core's end, the
//! broken invariant it reports as it stops, which the runtime hears as a VMM hears a core's
//! exit. Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
//! children are its fields, in order; one with no fields carries no packet. The GPU and the
//! files are devices of their own (gpu.json, fs.json). Majors 0x00 to 0x0F are xpute's
//! (ipc/frame.json); a program numbers its own protocols' commands past them.

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

/// The core's life as its runtime tells it, its turns, and its end.
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
    /// Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    /// A runtime has attached, the first or again after one before it went: the mount sends it
    /// once the front is laid. What the old runtime's requests leave behind is the core's to let go.
    SYS_ATTACH = 0x0102,
    /// The core is about to be frozen, its turns stopped for as long as its runtime is hidden:
    /// its last sure moment to save. A call, rung at once.
    SYS_SUSPEND = 0x0103,
    /// The core woken from SYS_SUSPEND. A call, rung at once.
    SYS_RESUME = 0x0104,
    /// A turn the scheduling policy grants, sent before it rings after what the program
    /// submits for it, as against a turn rung for a command alone: the wall clock, in Unix
    /// milliseconds. What else the policy says of the turn is the program's to send.
    SYS_TURN = 0x0105,

    // ---- sys, import ----
    /// The broken invariant the core reports as it stops (xpute-kit status/bug.rs).
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
