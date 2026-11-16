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
/// `table`
pub const TABLE: _rt::Method = 3;
/// `strtab`
pub const STRTAB: _rt::Method = 4;

pub trait Guest {
    fn start(&self) -> Result<Result<(), super::types::ErrorCode>, _rt::Errno>;
    fn stop(&self) -> Result<(), _rt::Errno>;
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, _rt::Errno>;
    fn table(&self, table: u32, from: u32) -> Result<Vec<u8>, _rt::Errno>;
    fn strtab(&self) -> Result<Vec<u8>, _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                START => match self.0.start() {
                    Ok(v) => {
                        ret_.start(2);
                        match &v {
                            Ok(_) => {}
                            Err(v) => {
                                ret_.u8(0, 1);
                                ret_.u8(1, *v as u8);
                            }
                        }
                        0
                    }
                    Err(e) => e as i32,
                },
                STOP => match self.0.stop() {
                    Ok(()) => 0,
                    Err(e) => e as i32,
                },
                STATS => {
                    let window_ms = args_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            args_.f64(8).map(Some)
                        }
                    })?;
                    match self.0.stats(window_ms) {
                        Ok(v) => {
                            ret_.start(8);
                            ret_.arr(0, &v);
                            0
                        }
                        Err(e) => e as i32,
                    }
                }
                TABLE => {
                    let table = args_.u32(0)?;
                    let from = args_.u32(4)?;
                    match self.0.table(table, from) {
                        Ok(v) => {
                            ret_.start(8);
                            ret_.arr(0, &v);
                            0
                        }
                        Err(e) => e as i32,
                    }
                }
                STRTAB => match self.0.strtab() {
                    Ok(v) => {
                        ret_.start(8);
                        ret_.arr(0, &v);
                        0
                    }
                    Err(e) => e as i32,
                },
                _ => _rt::Errno::ENOSYS as i32,
            })
        };
        run().unwrap_or_else(|e| e as i32)
    }
}

/// Mounts `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_guest::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag};
}
