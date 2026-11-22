// pxl-runtime/abi/object.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/object` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x4edab0;

/// `apply`
pub const APPLY: _rt::Method = 0;

pub trait Host {
    fn apply(&self, batch: _rt::Lent, task_return: _rt::Return<Result<(), _rt::ErrorCode>>);
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
                APPLY => {
                    let batch = _rt::Lent::load(args_, 0)?;
                    let side_ = self.0;
                    self.1.apply(
                        batch,
                        _rt::Return::new(
                            Box::new(move |stop| side_.on_cancel(tag_, stop)),
                            Box::new(move |got| match got {
                                Ok(v) => {
                                    side_.task_return(
                                        tag_,
                                        0,
                                        _rt::Packet::Args(Some((
                                            2,
                                            &|w: &mut _rt::Lower| match &v {
                                                Ok(_) => {}
                                                Err(v) => {
                                                    w.u8(0, 1);
                                                    w.u8(1, *v as u8);
                                                }
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
    pub(super) use xpute_host::abi::fs::types::ErrorCode;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::abi::types::Lent;
    pub(super) use xpute_link::{
        Handle, Lift, Lower, Method, Object, Packet, Return, Side, Tag, store_err,
    };
}
