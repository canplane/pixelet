// pxl-runtime/abi/object.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `pxl:core/object` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x4edab0;

/// `apply`
pub const APPLY: _rt::Method = 0;

pub trait Host {
    fn apply(&self, batch: _rt::Lent, task_return: _rt::Return<Result<(), _rt::ErrorCode>>);
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                APPLY => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let batch = _rt::Lent::load(r_, 0)?;
                    self.0.apply(
                        batch,
                        _rt::Return::new(
                            tag_,
                            _rt::link::on_cancel,
                            Box::new(move |got| {
                                let reply = match got {
                                    Ok(v) => _rt::Reply {
                                        result: 0,
                                        data: Some((
                                            2,
                                            Box::new(move |w: &mut _rt::Lower| match &v {
                                                Ok(v) => {}
                                                Err(v) => {
                                                    w.u8(0, 1);
                                                    w.u8(0 + 1, *v as u8);
                                                }
                                            }),
                                        )),
                                    },
                                    Err(e) => _rt::Reply::from(e as i32),
                                };
                                _rt::link::task_return(
                                    tag_,
                                    reply.result,
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                                );
                            }),
                        ),
                    );
                    _rt::Reply::from(_rt::Errno::EINPROGRESS as i32)
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `imp` under the interface's handle.
pub fn mount(imp: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(imp)));
}

mod _rt {
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::abi::types::Lent;
    pub use xpute_link::{Handle, Lift, Lower, Method, Object, Reply, Return, Tag};
    pub use xpute_native_runtime::abi::fs::types::ErrorCode;
    pub use xpute_native_runtime::link;
}
