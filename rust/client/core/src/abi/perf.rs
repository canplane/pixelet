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
use xpute_guest::wait::wait;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{
    ArrayElem, BranchCursor, Cursor, PacketWriter, ScalarType, SequenceType, TreeReader,
};
use xpute_link::{Handle, Method, Object, Reply, Return, Tag};

/// The interface's handle, named by `pxl:core/perf` (frame.json `NAMED`).
pub const HANDLE: Handle = 0x6be6ac;

/// The interface's qualified name: its package and its name.
pub const NAME: [&str; 2] = ["pxl:core", "perf"];

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
    fn call(&self, m: Method, tag: Tag, args: Option<BranchCursor<'_>>) -> Reply {
        let run = || -> Result<Reply, MarshalError> {
            Ok(match m {
                START => match self.0.start() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                STOP => match self.0.stop() {
                    Ok(v) => Reply::from(0),
                    Err(e) => Reply::from(e as i32),
                },
                STATS => {
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let window_ms = b.at(0).and_then(|c| {
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
                    let b = args.ok_or(MarshalError::new(Errno::EBADMSG))?;
                    let ring = b.at(0).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let from = b.at(1).and_then(|c| c.get_number().map(|v| v as u32))?;
                    let cnt = b.at(2).and_then(|c| c.get_number().map(|v| v as u32))?;
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

pub fn start() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, START, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn stop() -> Result<(), Errno> {
    let (r, data) = link::call(HANDLE, STOP, None);
    xpute_link::answer(r).map(|_| ())
}

pub fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
    let (r, data) = link::call(
        HANDLE,
        STATS,
        Some((1, &|w: &mut PacketWriter| match &window_ms {
            Some(v) => {
                w.f64(Some(*v as f64));
            }
            None => {
                w.nil();
            }
        })),
    );
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
            .map_err(|e| e.errnum)
    })
}

pub fn rings(ring: u32, from: u32, cnt: u32) -> Result<Vec<u8>, Errno> {
    let (r, data) = link::call(
        HANDLE,
        RINGS,
        Some((3, &|w: &mut PacketWriter| {
            w.u32(Some(ring as u32));
            w.u32(Some(from as u32));
            w.u32(Some(cnt as u32));
        })),
    );
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
            .map_err(|e| e.errnum)
    })
}

pub fn strtab() -> Result<Vec<u8>, Errno> {
    let (r, data) = link::call(HANDLE, STRTAB, None);
    xpute_link::answer(r).and_then(|_| {
        let p = data.as_deref().ok_or(Errno::EBADMSG)?;
        TreeReader::new(p)
            .and_then(|t| t.read_branch())
            .and_then(|b| b.at(0))
            .and_then(|c| c.get_array::<u8>().map(<[u8]>::to_vec))
            .map_err(|e| e.errnum)
    })
}
