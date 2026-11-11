// rust/dev/example/src/asker.spec.rs
//
// GENERATED from rust/dev/example/asker.json

//! The example's one protocol: the runtime sends SUM, and the core calls ASK as it adds.

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

use xpute_core::abi::number::{finite, integer, unknown};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- asker, export ----
    /// Carries two numbers. The core answers with their sum plus the number ASK answers.
    SUM = 0x1001,

    // ---- asker, import ----
    /// Asks the runtime for a number, which the core adds to the sum.
    ASK = 0x1002,
}

impl Command {
    pub const ALL: [Command; 2] = [Command::SUM, Command::ASK];

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
    pub const FIELDS: u32 = 2;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.a));
        w.u32(Some(self.b));
    }
}

/// Command::ASK's packet.
pub struct Ask {}

impl Ask {
    /// The children it is written as.
    pub const FIELDS: u32 = 0;

    pub fn write(&self, _: &mut PacketWriter) {}
}

impl xpute_core::io::door::Import for Ask {
    const NUMBER: u32 = 4098;
    const FIELDS: u32 = 0;
    const ACKREQ: bool = false;
    fn write(&self, w: &mut PacketWriter) {
        Ask::write(self, w);
    }
    fn named(&self) -> (u32, u32) {
        (0, 0)
    }
}

/// The asker protocol's core end: what it posts to the runtime and what it hears from it.
pub mod asker {
    use super::*;
    use xpute_core::io::door::{door, Outcome};
    use xpute_kit::status::errno::Errno;

    /// Command::ASK, answered by the runtime before this returns.
    pub fn ask() -> Result<u32, Errno> {
        door().call(&Ask {})
    }

    /// What the program does with each command the runtime sends.
    pub trait Exports {
        fn sum(&mut self, c: Sum) -> Outcome;
    }

    /// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
    pub fn apply<H: Exports>(h: &mut H, cmd: u32, pkt: &Option<&[u8]>) -> Outcome {
        match cmd {
            x if x == Command::SUM as u32 => h.sum(Sum::read(
                pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
            )?),
            _ => Err(MarshalError::new(Errno::ENOSYS)),
        }
    }
}
