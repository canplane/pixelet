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
/// `worst`
pub const WORST: _rt::Method = 4;
/// `strtab`
pub const STRTAB: _rt::Method = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Rows {
    pub first_seq: u64,
    pub end_seq: u64,
    pub bytes: Vec<u8>,
}

impl Rows {
    pub const SZ: usize = 24;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u64(at, self.first_seq);
        w.u64(at + 8, self.end_seq);
        w.arr(at + 16, &self.bytes);
    }

    /// Stores a record of these fields, each borrowed, as `store` stores one.
    pub fn store_of(w_: &mut _rt::Lower, at_: usize, first_seq: u64, end_seq: u64, bytes: &[u8]) {
        w_.u64(at_, first_seq);
        w_.u64(at_ + 8, end_seq);
        w_.arr(at_ + 16, bytes);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Rows, _rt::Errno> {
        Ok(Rows {
            first_seq: r.u64(at)?,
            end_seq: r.u64(at + 8)?,
            bytes: r.arr::<u8>(at + 16)?,
        })
    }
}

pub trait Guest {
    fn start(&self) -> Result<Result<(), super::types::ErrorCode>, _rt::XputeError>;
    fn stop(&self) -> Result<(), _rt::XputeError>;
    fn stats(&self, window_ms: Option<f64>) -> Result<Vec<u8>, _rt::XputeError>;
    fn table(&self, table: u32, from: u64, cnt: u32) -> Result<Rows, _rt::XputeError>;
    fn worst(&self) -> Result<Vec<u8>, _rt::XputeError>;
    fn strtab(&self) -> Result<Vec<u8>, _rt::XputeError>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let mut run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                START => {
                    let v = self.0.start()?;
                    {
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
                }
                STOP => {
                    let () = self.0.stop()?;
                    0
                }
                STATS => {
                    let window_ms = args_.case(0, 2).and_then(|d| {
                        if d == 0 {
                            Ok(None)
                        } else {
                            args_.f64(8).map(Some)
                        }
                    })?;
                    let v = self.0.stats(window_ms)?;
                    {
                        ret_.start(8);
                        ret_.arr(0, &v);
                        0
                    }
                }
                TABLE => {
                    let table = args_.u32(0)?;
                    let from = args_.u64(8)?;
                    let cnt = args_.u32(16)?;
                    let v = self.0.table(table, from, cnt)?;
                    {
                        ret_.start(Rows::SZ);
                        v.store(ret_, 0);
                        0
                    }
                }
                WORST => {
                    let v = self.0.worst()?;
                    {
                        ret_.start(8);
                        ret_.arr(0, &v);
                        0
                    }
                }
                STRTAB => {
                    let v = self.0.strtab()?;
                    {
                        ret_.start(8);
                        ret_.arr(0, &v);
                        0
                    }
                }
                _ => return Err(_rt::Errno::ENOSYS.into()),
            })
        };
        run().unwrap_or_else(|e| _rt::store_err(ret_, &e))
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
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag, store_err};
}
