// xpute-guest/abi/clocks/types.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

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
pub const NAME: [&str; 2] = ["xpute:clocks", "types"];

pub type Duration = u64;
