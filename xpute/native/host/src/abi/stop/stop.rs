// xpute-native-host/abi/stop/stop.rs
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

pub trait Stop {
    fn stopped(&self, errnum: i32, file: String, line: u32, n1: u64, n2: u64) -> Result<(), Errno>;
}

struct StopObject(Rc<dyn Stop>);

impl Object for StopObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                STOPPED => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let errnum = b_.at(0).and_then(|c| c.get_number().map(|v| v as i32))?;
                    let file = b_.at(1).and_then(|c| c.get_str_ref().map(str::to_owned))?;
                    let line = b_.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let n1 = b_.at(3).and_then(|c| c.get_u64())?;
                    let n2 = b_.at(4).and_then(|c| c.get_u64())?;
                    match self.0.stopped(errnum, file, line, n1, n2) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Stop>) {
    link::mount_named(HANDLE, Rc::new(StopObject(imp)));
}
