// pxl-core/abi/watch.rs
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
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/watch` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x71919e;

/// `start`
pub const START: Method = 0;
/// `stop`
pub const STOP: Method = 1;

pub fn start(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        START,
        Some((1, &|w_: &mut PacketWriter| {
            w_.u32(Some(sensor as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn stop(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        STOP,
        Some((1, &|w_: &mut PacketWriter| {
            w_.u32(Some(sensor as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
