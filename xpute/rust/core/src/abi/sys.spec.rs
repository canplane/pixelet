// xpute-core/abi/sys.spec.rs
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
//! Each command is `major << 8 | minor` (kit abi/cmd). Its packet is an XTP branch whose
//! children are the command's fields, in order; a command with no fields carries no packet.
//! The GPU and the files are devices with protocols of their own (gpu.json, fs.json).
//! Majors 0x00 to 0x0F are xpute's (io/frame.json), and a program numbers the commands of
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
    FRAME = 0x06,
    PACE = 0x07,
}

impl SysMinor {
    pub const ALL: [SysMinor; 7] = [
        SysMinor::ABORT,
        SysMinor::ATTACH,
        SysMinor::SUSPEND,
        SysMinor::RESUME,
        SysMinor::TURN,
        SysMinor::FRAME,
        SysMinor::PACE,
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
    /// The channel's own test. The reply carries the packet's value, or 0 when the frame has no
    /// packet.
    NOP = 0x0000,
    /// A runtime has attached, for the first time or after an earlier runtime went away. The
    /// runtime sends it once it has laid the front of the core's memory (io/head.json). The core
    /// lets go of whatever the earlier runtime's requests left behind.
    SYS_ATTACH = 0x0102,
    /// The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
    /// is the core's last sure moment to save. The UI sends it and does not wait; the runtime
    /// lets it reach the core in a turn before turns stop.
    SYS_SUSPEND = 0x0103,
    /// The core has been woken after SYS_SUSPEND.
    SYS_RESUME = 0x0104,
    /// Marks a turn that the scheduling policy grants, as opposed to a turn run only because a
    /// command arrived. The runtime submits it after what the program submits for that turn,
    /// and before it runs the turn. It carries the wall clock in Unix milliseconds. What frame pacing
    /// says of the turn comes before it (SYS_FRAME); anything else the policy has to say, the
    /// program sends in commands of its own.
    SYS_TURN = 0x0105,
    /// The display frame frame pacing grants the next SYS_TURN at (@xpute/runtime
    /// sched/frame_pacing.ts, xpute-runtime native/pacing.rs): the display's refresh, the interval
    /// turns are paced at, a whole number of refreshes and the frame's budget, and whether the
    /// turn follows an input. A runtime whose policy paces no display sends none.
    SYS_FRAME = 0x0106,

    // ---- sys, import ----
    /// Reports the broken invariant the core stops on (xpute-kit status/bug.rs).
    SYS_ABORT = 0x0101,
    /// The least interval the core asks between its turns, 0 for none: its load's word on the
    /// rate, which the next SYS_FRAME's `paced_ms` answers, a whole number of refreshes. A policy
    /// that paces no display ignores it.
    SYS_PACE = 0x0107,
}

impl Command {
    pub const ALL: [Command; 8] = [
        Command::NOP,
        Command::SYS_ATTACH,
        Command::SYS_SUSPEND,
        Command::SYS_RESUME,
        Command::SYS_TURN,
        Command::SYS_FRAME,
        Command::SYS_ABORT,
        Command::SYS_PACE,
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
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.val));
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
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.unix_ms));
    }
}

/// Command::SYS_FRAME's packet.
#[derive(Clone, Copy, Debug)]
pub struct SysFrame {
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
}

impl SysFrame {
    pub fn read(pkt: &[u8]) -> Result<SysFrame, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysFrame {
            refresh_ms: finite(f.at(0)?.get_number()?)?,
            paced_ms: finite(f.at(1)?.get_number()?)?,
            interactive: f.at(2)?.get_bool()?,
        })
    }
    pub const FIELDS: u32 = 3;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.refresh_ms));
        w.f64(Some(self.paced_ms));
        w.bool(Some(self.interactive));
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

impl SysAbort<'_> {
    /// As the runtime beside the core reads it.
    pub fn parts(pkt: &[u8]) -> Result<(i32, u32, u32, u32, u32, u32, u32, &str), MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok((
            integer::<i32>(f.at(0)?.get_number()?)?,
            integer::<u32>(f.at(1)?.get_number()?)?,
            integer::<u32>(f.at(2)?.get_number()?)?,
            integer::<u32>(f.at(3)?.get_number()?)?,
            integer::<u32>(f.at(4)?.get_number()?)?,
            integer::<u32>(f.at(5)?.get_number()?)?,
            integer::<u32>(f.at(6)?.get_number()?)?,
            f.at(7)?.get_str_ref()?,
        ))
    }
}

impl crate::io::door::Import for SysAbort<'_> {
    const NUMBER: u32 = 257;
    const FIELDS: u32 = 8;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        SysAbort::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// Command::SYS_PACE's packet.
pub struct SysPace {
    pub interval_ms: f64,
}

impl SysPace {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.f64(Some(self.interval_ms));
    }
}

impl SysPace {
    /// As the runtime beside the core reads it.
    pub fn read(pkt: &[u8]) -> Result<SysPace, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(SysPace {
            interval_ms: finite(f.at(0)?.get_number()?)?,
        })
    }
}

impl crate::io::door::Import for SysPace {
    const NUMBER: u32 = 263;
    const FIELDS: u32 = 1;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        SysPace::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// The sys protocol's core end: what it posts to the runtime and what it hears from it.
use crate::io::door::{door, Outcome};
use xpute_kit::status::errno::Errno;

/// Command::SYS_ABORT, posted for the runtime.
pub fn sys_abort(e: &SysAbort<'_>) {
    door().send(e);
}

/// Command::SYS_PACE, posted for the runtime.
pub fn sys_pace(e: &SysPace) {
    door().send(e);
}

/// What the program does with each command the runtime sends.
pub trait Exports {
    fn nop(&mut self, c: Nop) -> Outcome;
    fn sys_attach(&mut self) -> Outcome;
    fn sys_suspend(&mut self) -> Outcome;
    fn sys_resume(&mut self) -> Outcome;
    fn sys_turn(&mut self, c: SysTurn) -> Outcome;
    fn sys_frame(&mut self, c: SysFrame) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn apply<H: Exports>(h: &mut H, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
    match cmd {
        x if x == Command::NOP as u32 => h.nop(Nop::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::SYS_ATTACH as u32 => h.sys_attach(),
        x if x == Command::SYS_SUSPEND as u32 => h.sys_suspend(),
        x if x == Command::SYS_RESUME as u32 => h.sys_resume(),
        x if x == Command::SYS_TURN as u32 => h.sys_turn(SysTurn::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::SYS_FRAME as u32 => h.sys_frame(SysFrame::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}
