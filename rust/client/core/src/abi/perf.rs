// pxl-core/abi/perf.rs
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

/// The interface's handle, named by `pxl:core/perf` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x6be6ac;

/// `start`
pub const START: Method = 0;
/// `stop`
pub const STOP: Method = 1;
/// `stats`
pub const STATS: Method = 2;
/// `rings`
pub const RINGS: Method = 3;
/// `strtab`
pub const STRTAB: Method = 4;

pub trait Perf {
    fn start(&self) -> Result<(), Errno>;
    fn stop(&self) -> Result<(), Errno>;
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, Errno>;
    fn rings(&self, ring: u32, from: u32, cnt: u32) -> Result<Vec<u8>, Errno>;
    fn strtab(&self) -> Result<Vec<u8>, Errno>;
}

struct PerfObject(Rc<dyn Perf>);

impl Object for PerfObject {
    fn call(&self, m_: Method, tag_: Tag, args_: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m_ {
                START => match self.0.start() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                STOP => match self.0.stop() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                STATS => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let window_ms = b_.at(0).and_then(|c| {
                        c.opt()
                            .map(|c| c.get_number().map(|v| v as f64))
                            .transpose()
                    })?;
                    match self.0.stats(window_ms) {
                        Ok(v) => Reply {
                            result: 0,
                            data: Some((
                                1,
                                Box::new(move |w: &mut PacketWriter| {
                                    w.array::<u8>(Some(&*v));
                                }),
                            )),
                        },
                        Err(e) => Reply::from(e as i32),
                    }
                }
                RINGS => {
                    let b_ = args_.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let ring = b_.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let from = b_.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let cnt = b_.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
                    match self.0.rings(ring, from, cnt) {
                        Ok(v) => Reply {
                            result: 0,
                            data: Some((
                                1,
                                Box::new(move |w: &mut PacketWriter| {
                                    w.array::<u8>(Some(&*v));
                                }),
                            )),
                        },
                        Err(e) => Reply::from(e as i32),
                    }
                }
                STRTAB => match self.0.strtab() {
                    Ok(v) => Reply {
                        result: 0,
                        data: Some((
                            1,
                            Box::new(move |w: &mut PacketWriter| {
                                w.array::<u8>(Some(&*v));
                            }),
                        )),
                    },
                    Err(e) => Reply::from(e as i32),
                },
                _ => Reply::from(Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| Reply::from(e.errnum as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: Rc<dyn Perf>) {
    link::mount_named(HANDLE, Rc::new(PerfObject(imp)));
}
