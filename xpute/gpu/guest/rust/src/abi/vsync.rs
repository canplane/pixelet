// xpute-gpu-guest/abi/vsync.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use core::future::Future;
use std::rc::Rc;
use xpute_guest::link;
use xpute_guest::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x42eda6;

/// `next`
pub const NEXT: Method = 0;

pub fn next() -> impl Future<Output = Result<(), Errno>> {
    let sent = wait::wait(HANDLE, NEXT, None, ());
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
