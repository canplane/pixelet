// pxl-runtime/abi/watch.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/watch` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x71919e;

/// `start`
pub const START: _rt::Method = 0;
/// `stop`
pub const STOP: _rt::Method = 1;

pub trait Host {
    fn start(&self, sensor: super::types::Sensor) -> Result<(), _rt::Errno>;
    fn stop(&self, sensor: super::types::Sensor) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Host>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                START => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let sensor = r_
                        .case(0, 2)
                        .and_then(|v| super::types::Sensor::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    match self.0.start(sensor) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                STOP => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let sensor = r_
                        .case(0, 2)
                        .and_then(|v| super::types::Sensor::of(v).ok_or(_rt::Errno::EBADMSG))?;
                    match self.0.stop(sensor) {
                        Ok(()) => _rt::Reply::from(0),
                        Err(e) => _rt::Reply::from(e as i32),
                    }
                }
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Host>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Method, Object, Reply, Tag};
    pub(super) use xpute_native_runtime::link;
}
