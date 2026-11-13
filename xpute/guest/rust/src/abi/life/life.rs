// xpute-guest/abi/life/life.rs
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
use crate::wait::wait;
use std::rc::Rc;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `xpute:life/life` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x46edb9;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["xpute:life", "life"];

/// `hidden`
pub const HIDDEN: Method = 0;
/// `shown`
pub const SHOWN: Method = 1;

pub trait Life {
    fn hidden(&self) -> Result<(), Errno>;
    fn shown(&self) -> Result<(), Errno>;
}

struct LifeObject(Rc<dyn Life>);

impl Object for LifeObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                HIDDEN => match self.0.hidden() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                SHOWN => match self.0.shown() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Life>) {
    link::mount_named(HANDLE, Rc::new(LifeObject(imp)));
}

pub fn hidden() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, HIDDEN, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn shown() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, SHOWN, None);
    xpute_link::answer(r).map(|_| ())
}
