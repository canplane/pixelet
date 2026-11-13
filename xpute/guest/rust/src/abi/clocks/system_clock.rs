// xpute-guest/abi/clocks/system_clock.rs
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
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:clocks/system-clock` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x7dfa61;

/// `now`
pub const NOW: Method = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct Instant {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl Instant {
    pub fn write(&self, w: &mut PacketWriter) {
        w.branch(2, |w| {
            w.i64(Some(self.seconds));
            w.u32(Some(self.nanoseconds as u32));
        });
    }

    pub fn read(c: Cursor<'_>) -> Result<Instant, MarshalError> {
        let b = c.as_branch()?;
        Ok(Instant {
            seconds: b.at(0).and_then(|c| c.get_i64())?,
            nanoseconds: b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?,
        })
    }
}

pub fn now() -> Result<Instant, Errno> {
    let (r, data) = link::call(HANDLE, NOW, None);
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| Instant::read(c))
            .map_err(|e| e.errnum)
    })
}
