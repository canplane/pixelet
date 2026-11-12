// pxl-core/abi/watch.rs
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

use std::cell::Cell;
use std::rc::Rc;
use xpute_core::link::{self, Handle, Method, Object, Reply, Return, Tag};
use xpute_core::wait::wait;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, TreeReader};

/// The singleton's URI.
pub const URI: [&str; 2] = ["pxl:core", "watch"];

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

/// Mounts `imp` at the URI, one object for every open.
pub fn mount(imp: Rc<dyn Watch>) {
    let o: Rc<dyn Object> = Rc::new(WatchObject(imp));
    link::mount(link::uri(&URI), move |_, _| Ok(o.clone()));
}

thread_local! {
    static HANDLE: Cell<Option<Handle>> = const { Cell::new(None) };
}

/// Opens the singleton on the other side, once, at boot.
pub fn open() -> Result<(), Errno> {
    HANDLE.set(Some(link::open(&link::uri(&URI))?));
    Ok(())
}

/// The singleton's handle, which `open` opened.
pub fn handle() -> Handle {
    HANDLE.get().or_bug(Errno::EBADF)
}

pub fn start(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        START,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
        })),
    );
    link::answer(r).map(|_| ())
}

pub fn stop(sensor: super::types::Sensor) -> Result<(), Errno> {
    let (r, data) = link::call(
        handle(),
        STOP,
        Some((1, &|w: &mut PacketWriter| {
            w.u32(Some(sensor as u32));
        })),
    );
    link::answer(r).map(|_| ())
}
