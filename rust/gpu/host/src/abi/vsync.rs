// pxl-gpu-host/abi/vsync.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/vsync` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x51cb2a;

/// `next`
pub const NEXT: _rt::Method = 0;

pub trait Host {
    fn next(&self, task_return: _rt::Return<()>);
}

struct _Object(&'static _rt::Side, _rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, tag_: _rt::Tag, _: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                NEXT => {
                    let side_ = self.0;
                    self.1.next(_rt::Return::new(
                        Box::new(move |stop| side_.on_cancel(tag_, stop)),
                        Box::new(move |got| {
                            let reply = match got {
                                Ok(()) => _rt::Reply::from(0),
                                Err(e) => _rt::Reply::from(e as i32),
                            };
                            side_.task_return(
                                tag_,
                                reply.result,
                                _rt::Packet::Args(
                                    reply
                                        .data
                                        .as_ref()
                                        .map(|(n, w)| (*n, &**w as &dyn Fn(&mut _rt::Lower))),
                                ),
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

/// Serves `implementation` under the interface's handle on `side_`.
pub fn mount(side_: &'static _rt::Side, implementation: _rt::Rc<dyn Host>) {
    side_.mount_named(HANDLE, _rt::Rc::new(_Object(side_, implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lower, Method, Object, Packet, Reply, Return, Side, Tag};
}
