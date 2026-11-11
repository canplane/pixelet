// xpute-runtime/abi/sys.spec.rs
//
// GENERATED from spec/abi/sys.json

//! xpute's own protocol on majors NOP and SYS. Through it the runtime tells the core about
//! the core's life and its turns, and the core reports its end. NOP is the channel's own test.
//! SYS_ATTACH, SYS_SUSPEND and SYS_RESUME tell the core that a runtime attached, for the
//! first time or again, that the core is about to be frozen, and that it was woken; what
//! each one means the core keeps or resets is the core's to decide. SYS_TURN marks a turn
//! that the runtime's scheduling policy grants, and carries the wall clock; SYS_FRAME and
//! SYS_PACE carry frame pacing's grant and the core's asked rate. SYS_ABORT
//! reports the broken invariant the core stops on, the last frame the core sends.
//!
//! Each command is `major << 8 | minor` (io/frame.json). Its packet is an XTP branch whose
//! children are the command's fields, in order; a command with no fields carries no packet.
//! The GPU and the files are devices with protocols of their own (gpu.json, fs.json).
//! Majors 0x00 to 0x0F are xpute's (io/frame.json), and a program numbers the commands of
//! its own protocols past them.

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

use crate::abi::{Export, Outcome};
use xpute_core::abi::number::{finite, integer, unknown};
pub use xpute_core::abi::sys::{Command, Major, SysMinor};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};

/// Command::NOP's packet.
pub struct Nop {
    pub val: u32,
}

impl Nop {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.val));
    }
}

impl Export for Nop {
    const NUMBER: u32 = 0;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        Nop::write(self, w);
    }
}

/// Command::SYS_ATTACH's packet.
pub struct SysAttach {}

impl SysAttach {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for SysAttach {
    const NUMBER: u32 = 258;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        SysAttach::write(self, w);
    }
}

/// Command::SYS_SUSPEND's packet.
pub struct SysSuspend {}

impl SysSuspend {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for SysSuspend {
    const NUMBER: u32 = 259;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        SysSuspend::write(self, w);
    }
}

/// Command::SYS_RESUME's packet.
pub struct SysResume {}

impl SysResume {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl Export for SysResume {
    const NUMBER: u32 = 260;
    const FIELDS: u32 = 0;
    fn write(&self, w: &mut PacketWriter) {
        SysResume::write(self, w);
    }
}

/// Command::SYS_TURN's packet.
pub struct SysTurn {
    pub unix_ms: f64,
}

impl SysTurn {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.unix_ms));
    }
}

impl Export for SysTurn {
    const NUMBER: u32 = 261;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SysTurn::write(self, w);
    }
}

/// Command::SYS_FRAME's packet.
pub struct SysFrame {
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
}

impl SysFrame {
    /// The children it is written as.
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.refresh_ms));
        w.f64(Some(self.paced_ms));
        w.bool(Some(self.interactive));
    }
}

impl Export for SysFrame {
    const NUMBER: u32 = 262;
    const FIELDS: u32 = 3;
    fn write(&self, w: &mut PacketWriter) {
        SysFrame::write(self, w);
    }
}

/// Command::SYS_ABORT's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysAbort<'a> {
    pub errnum: i32,
    pub line: u32,
    pub column: u32,
    pub a_low: u32,
    pub a_high: u32,
    pub b_low: u32,
    pub b_high: u32,
    pub file: &'a str,
}

impl<'a> SysAbort<'a> {
    pub fn read(pkt: &'a [u8]) -> Result<SysAbort<'a>, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysAbort {
            errnum: integer::<i32>(f.at(0)?.get_number()?)?,
            line: integer::<u32>(f.at(1)?.get_number()?)?,
            column: integer::<u32>(f.at(2)?.get_number()?)?,
            a_low: integer::<u32>(f.at(3)?.get_number()?)?,
            a_high: integer::<u32>(f.at(4)?.get_number()?)?,
            b_low: integer::<u32>(f.at(5)?.get_number()?)?,
            b_high: integer::<u32>(f.at(6)?.get_number()?)?,
            file: f.at(7)?.get_str_ref()?,
        })
    }
}

/// Command::SYS_PACE's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysPace {
    pub interval_ms: f64,
}

impl SysPace {
    pub fn read(pkt: &[u8]) -> Result<SysPace, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysPace {
            interval_ms: finite(f.at(0)?.get_number()?)?,
        })
    }
}

/// The majors the runtime routes the protocol's imports by, to `serve`.
pub const MAJORS: [u32; 1] = [Major::SYS as u32];

/// What the runtime does with each command the core posts.
pub trait Imports {
    fn sys_abort(&mut self, e: SysAbort<'_>) -> Outcome;
    fn sys_pace(&mut self, e: SysPace) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn serve<H: Imports>(
    h: &mut H,
    _tag: u32,
    cmd: u32,
    pkt: &Option<&[u8]>,
    _named: &[u8],
) -> Outcome {
    match cmd {
        x if x == Command::SYS_ABORT as u32 => h.sys_abort(SysAbort::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::SYS_PACE as u32 => h.sys_pace(SysPace::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}
