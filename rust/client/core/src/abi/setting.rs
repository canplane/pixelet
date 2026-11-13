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
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/setting` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x6f63cb;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "setting"];

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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                DEV => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let dev = b.at(0).and_then(|c| c.get_bool())?;
                    match self.0.dev(dev) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                SET => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let key = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let val = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
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

pub fn dev(dev: bool) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        DEV,
        Some((1, &|w: &mut PacketWriter| {
            w.bool(Some(dev));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn set(key: u32, val: u32) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        SET,
        Some((2, &|w: &mut PacketWriter| {
            w.u32(Some(key as u32));
            w.u32(Some(val as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
