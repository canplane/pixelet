// xpute-gpu-native-host/abi/vsync.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::all,
    reason = "generated from WIT"
)]

/// The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x42eda6;

/// `next`
pub const NEXT: _rt::Method = 0;

pub trait Host {
    fn next(&self, task_return: _rt::Return<()>);
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                NEXT => {
                    self.0.next(_rt::Return::new(
                        tag_,
                        _rt::link::on_cancel,
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(v) => _rt::Reply::from(0),
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
                    ));
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
    pub use xpute_link::{Handle, Lower, Method, Object, Reply, Return, Tag};
    pub use xpute_native_runtime::link;
}
