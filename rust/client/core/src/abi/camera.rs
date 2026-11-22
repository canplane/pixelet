// pxl-core/abi/camera.rs
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x5a7108;

/// `track`
pub const TRACK: _rt::Method = 0;
/// `goto`
pub const GOTO: _rt::Method = 1;

pub trait Guest {
    fn track(&self, mode: super::types::TrackingMode) -> Result<(), _rt::XputeError>;
    fn goto(&self, lon: f64, lat: f64) -> Result<(), _rt::XputeError>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                TRACK => {
                    let mode = args_.case(0, 3).and_then(|v| {
                        super::types::TrackingMode::of(v).ok_or(_rt::Errno::EBADMSG)
                    })?;
                    let () = self.0.track(mode)?;
                    0
                }
                GOTO => {
                    let lon = args_.f64(0)?;
                    let lat = args_.f64(8)?;
                    let () = self.0.goto(lon, lat)?;
                    0
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
