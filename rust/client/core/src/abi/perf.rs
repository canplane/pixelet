// pxl-core/abi/perf.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/perf` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x6be6ac;

/// `start`
pub const START: _rt::Method = 0;
/// `stop`
pub const STOP: _rt::Method = 1;
/// `stats`
pub const STATS: _rt::Method = 2;
/// `ring`
pub const RING: _rt::Method = 3;
/// `strtab`
pub const STRTAB: _rt::Method = 4;

pub trait Guest {
    fn start(&self) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
    fn stop(&self) -> Result<(), _rt::Errno>;
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, _rt::Errno>;
    fn ring(&self, ring: u32, from: u32, cnt: u32) -> Result<Vec<u8>, _rt::Errno>;
    fn strtab(&self) -> Result<Vec<u8>, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                START => match self.0.start() {
                    Ok(v) => _rt::Reply {
                        result: 0,
                        data: Some((
                            2,
                            Box::new(move |w: &mut _rt::Lower| match &v {
                                Ok(_) => {}
                                Err(v) => {
                                    w.u8(0, 1);
                                    w.u8(1, *v as u8);
                                }
                            }),
                        )),
                    },
                    Err(e) => _rt::Reply::from(e as i32),
                },
                STOP => match self.0.stop() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                STATS => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let window_ms = r_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            r_.f64(8).map(Some)
                        }
                    })?;
                    match self.0.stats(window_ms) {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                8,
                                Box::new(move |w: &mut _rt::Lower| {
                                    w.arr(0, &v);
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                RING => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let ring = r_.u32(0)?;
                    let from = r_.u32(4)?;
                    let cnt = r_.u32(8)?;
                    match self.0.ring(ring, from, cnt) {
                        Ok(v) => _rt::Reply {
                            result: 0,
                            data: Some((
                                8,
                                Box::new(move |w: &mut _rt::Lower| {
                                    w.arr(0, &v);
                                }),
                            )),
                        },
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                STRTAB => match self.0.strtab() {
                    Ok(v) => _rt::Reply {
                        result: 0,
                        data: Some((
                            8,
                            Box::new(move |w: &mut _rt::Lower| {
                                w.arr(0, &v);
                            }),
                        )),
                    },
                    Err(e) => _rt::Reply::from(e as i32),
                },
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub use std::rc::Rc;
    pub use xpute_guest::link;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Tag};
}
