// pxl-core/abi/watch.rs
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

/// The interface's handle, named by `pxl:core/watch` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x71919e;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "watch"];

/// `start`
pub const START: Method = 0;
/// `stop`
pub const STOP: Method = 1;

pub trait Watch {
    fn start(&self, sensor: super::types::Sensor) -> Result<(), Errno>;
    fn stop(&self, sensor: super::types::Sensor) -> Result<(), Errno>;
}

struct WatchObject(Rc<dyn Watch>);

impl Object for WatchObject {
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                START => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    match self.0.start(sensor) {
                        Ok(v) => Reply::from(0),
                        Err(e) => Reply::from(e as i32),
                    }
                }
                STOP => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let sensor = b.at(0).and_then(|c| {
                        c.get_number().and_then(|v| {
                            super::types::Sensor::of(v as u32)
                                .ok_or(MarshalError::new(Errno::EBADMSG))
                        })
                    })?;
                    match self.0.stop(sensor) {
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
pub fn mount(imp: Rc<dyn Watch>) {
    link::mount_named(HANDLE, Rc::new(WatchObject(imp)));
}

pub fn start(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        START,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}

pub fn stop(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        HANDLE,
        STOP,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
        })),
    );
    xpute_link::answer(r).map(|_| ())
}
