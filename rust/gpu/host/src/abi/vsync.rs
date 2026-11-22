// pxl-gpu-host/abi/vsync.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/vsync` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x51cb2a;

/// `next`
pub const NEXT: _rt::Method = 0;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub at: u64,
    pub gap: u64,
}

impl Frame {
    pub const SZ: usize = 16;
    pub const ALIGN: usize = 8;

    pub fn store(&self, w: &mut _rt::Lower, at: usize) {
        w.u64(at, self.at);
        w.u64(at + 8, self.gap);
    }

    /// Stores a record of these fields, each borrowed, as `store` stores one.
    pub fn store_of(w_: &mut _rt::Lower, at_: usize, at: u64, gap: u64) {
        w_.u64(at_, at);
        w_.u64(at_ + 8, gap);
    }

    pub fn load(r: _rt::Lift<'_>, at: usize) -> Result<Frame, _rt::Errno> {
        Ok(Frame {
            at: r.u64(at)?,
            gap: r.u64(at + 8)?,
        })
    }
}

pub trait Host {
    fn next(&self, after: u64, task_return: _rt::Return<Frame>);
}

struct _Object(&'static _rt::Side, _rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(
        &self,
        m_: _rt::Method,
        tag_: _rt::Tag,
        args_: _rt::Lift,
        ret_: &mut _rt::Lower,
    ) -> i32 {
        let run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                NEXT => {
                    let after = args_.u64(0)?;
                    let side_ = self.0;
                    self.1.next(
                        after,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(v) => {
                                    side_.task_return(
                                        tag_,
                                        0,
                                        _rt::Packet::Args(Some((
                                            Frame::SZ,
                                            &|w: &mut _rt::Lower| {
                                                v.store(w, 0);
                                            },
                                        ))),
                                    );
                                }
                                Err(e) => {
                                    side_.task_err(tag_, e);
                                }
                            }),
                        ),
                    );
                    _rt::Errno::EINPROGRESS as i32
                }
                _ => return Err(_rt::Errno::ENOSYS.into()),
            })
        };
        run().unwrap_or_else(|e| _rt::store_err(ret_, &e))
    }
}

/// Mounts `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(side_, implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{
        Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag, store_err,
    };
}
