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

pub trait Life {
    fn hidden(&self) -> Result<(), Errno>;
    fn shown(&self) -> Result<(), Errno>;
}

struct LifeObject(Rc<dyn Life>);

impl Object for LifeObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
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
