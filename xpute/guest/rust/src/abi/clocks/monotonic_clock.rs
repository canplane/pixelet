// xpute-guest/abi/clocks/monotonic_clock.rs
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use crate::link;
use crate::wait;
use core::future::Future;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x73f703;

/// `now`
pub const NOW: Method = 0;
/// `wait-for`
pub const WAIT_FOR: Method = 1;

pub type Mark = u64;

pub fn now() -> Result<u64, Errno> {
    let (r, data) = link::call(HANDLE, NOW, None);
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_u64())
            .map_err(|e| e.errnum)
    })
}

pub fn wait_for(how_long: u64) -> impl Future<Output = Result<(), Errno>> {
    let sent = wait::wait(
        HANDLE,
        WAIT_FOR,
        Some((1, &|w_: &mut PacketWriter| {
            w_.u64(Some(how_long));
        })),
        (),
    );
    async move {
        let (r, data, lent) = sent.await;
        xpute_link::answer(r).map(|_| ())
    }
}
