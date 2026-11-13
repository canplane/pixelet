// pxl-core/abi/types.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "types"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Sensor {
    Geolocation = 0,
    Orientation = 1,
}

impl Sensor {
    /// Every member, in its source's own order.
    pub const ALL: [Sensor; 2] = [Sensor::Geolocation, Sensor::Orientation];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u32) -> Option<Sensor> {
        match val {
            0 => Some(Sensor::Geolocation),
            1 => Some(Sensor::Orientation),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Sensor::Geolocation => "Geolocation",
            Sensor::Orientation => "Orientation",
        }
    }
}
