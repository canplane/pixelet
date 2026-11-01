// xpute-example/cmd.spec.rs
//
// GENERATED from spec/example/cmd.json

//! The example's own commands (rust/example, ts/example/sum.test.ts): a program's
//! table, from which its guest's readers and its host's writers are generated.

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

use xpute_guest::abi::number::{finite, integer, unknown};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// The example's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    /// Two numbers, answered with their sum and the number the host holds.
    SUM = 0x1001,
    /// Raised as SUM is answered: the two numbers' sum, before the host's is added.
    SUMMED = 0x1002,
}

impl Command {
    pub const ALL: [Command; 2] = [Command::SUM, Command::SUMMED];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::SUM's packet.
#[derive(Clone, Copy, Debug)]
pub struct Sum {
    pub a: u32,
    pub b: u32,
}

impl Sum {
    pub fn read(pkt: &[u8]) -> Result<Sum, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(Sum {
            a: integer::<u32>(f.at(0)?.get_number()?)?,
            b: integer::<u32>(f.at(1)?.get_number()?)?,
        })
    }
}

impl<'a> xpute_guest::program::FromPacket<'a> for Sum {
    fn from_packet(pkt: xpute_guest::program::Packet<'a>) -> Result<Self, Errno> {
        Sum::read(pkt.bytes().ok_or(Errno::EBADMSG)?).map_err(|e| e.errnum)
    }
}

/// Command::SUMMED's event.
pub struct SummedEvent {
    pub total: u32,
}

impl SummedEvent {
    /// The children it is written as.
    pub const FIELDS: u32 = 1;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.total));
    }
}

impl xpute_guest::program::Event for SummedEvent {
    const NUMBER: u32 = 4098;
    const FIELDS: u32 = 1;
    fn write(&self, w: &mut PacketWriter) {
        SummedEvent::write(self, w);
    }
}
