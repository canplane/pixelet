// xpute-guest/abi/stop/stop.rs
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

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

/// The interface's handle, named by `xpute:stop/stop` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x4eccaa;

/// `stopped`
pub const STOPPED: Method = 0;

pub fn stopped(errnum: i32, file: &str, line: u32, n1: u64, n2: u64) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        STOPPED,
        Some((5, &|w_: &mut PacketWriter| {
            w_.i32(Some(errnum as i32));
            w_.str(Some(&*file));
            w_.u32(Some(line as u32));
            w_.u64(Some(n1));
            w_.u64(Some(n2));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
