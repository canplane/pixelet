// pxl-core/abi/types.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensor {
    #[default]
    Geolocation = 0,
    Orientation = 1,
}

impl Sensor {
    pub const ALL: [Sensor; 2] = [Sensor::Geolocation, Sensor::Orientation];

    pub fn of(v: u32) -> Option<Sensor> {
        Sensor::ALL.get(v as usize).copied()
    }
}
