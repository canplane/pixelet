// xpute-guest/abi/cmd.spec.rs
//
// GENERATED from spec/abi/cmd.json — do not edit.

//! Every program's commands, by number (kit abi/cmd: `major << 8 | minor`), and the
//! packet each carries — an XTP branch whose children are its fields, in order. A
//! command with no fields carries no packet. The majors below PROGRAM_MAJOR are xpute's;
//! a program numbers its own from there, in a table of its own that names this one as
//! its base, so the two never give out one number twice.
//!
//! The direction is the ring: `packet` is what the host submits, `signal` what the guest
//! raises on the completion ring.

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no signal or no enum read off the wire leaves a writer or a
// number's check unused.
#![allow(non_camel_case_types, clippy::type_complexity, unused_imports, dead_code)]

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

/// The system calls' answers (ipc/sys.json).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SysMinor {
    OPENED = 0x01,
}

impl SysMinor {
    pub const ALL: [SysMinor; 1] = [SysMinor::OPENED];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SysMinor> {
        SysMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every program's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    /// Replies with its packet's value, or 0 where it has none — the rings' own test.
    NOP = 0x0000,
    /// An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
    /// Never sent for a descriptor already closed.
    SYS_OPENED = 0x0101,
}

impl Command {
    pub const ALL: [Command; 2] = [Command::NOP, Command::SYS_OPENED];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::NOP's packet.
#[derive(Clone, Copy, Debug)]
pub struct Nop {
    pub value: u32,
}

impl Nop {
    pub fn read(pkt: &[u8]) -> Result<Nop, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(Nop {
            value: integer::<u32>(f.at(0)?.get_number()?)?,
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
