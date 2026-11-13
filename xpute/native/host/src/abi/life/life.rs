// xpute-native-host/abi/life/life.rs
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link;
use crate::wait;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:life/life` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x46edb9;

/// `hidden`
pub const HIDDEN: Method = 0;
/// `shown`
pub const SHOWN: Method = 1;

pub fn hidden() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, HIDDEN, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn shown() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, SHOWN, None);
    xpute_link::answer(r).map(|_| ())
}
