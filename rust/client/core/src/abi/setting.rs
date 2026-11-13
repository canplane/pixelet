// pxl-core/abi/setting.rs
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

/// The interface's handle, named by `pxl:core/setting` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x6f63cb;

/// `dev`
pub const DEV: Method = 0;
/// `set`
pub const SET: Method = 1;

pub trait Setting {
    fn dev(&self, dev: bool) -> Result<(), Errno>;
    fn set(&self, key: u32, val: u32) -> Result<(), Errno>;
}

struct SettingObject(Rc<dyn Setting>);

impl Object for SettingObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                DEV => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let dev = b_.at(0).and_then(|c| c.get_bool())?;
                    match self.0.dev(dev) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                SET => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let key = b_.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let val = b_.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                    match self.0.set(key, val) {
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
pub fn mount(imp: Rc<dyn Setting>) {
    link::mount_named(HANDLE, Rc::new(SettingObject(imp)));
}
