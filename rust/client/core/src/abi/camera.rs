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
    fn track(&self, mode: super::types::TrackingMode) -> Result<(), _rt::Errno>;
    fn goto(&self, lon: f64, lat: f64) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                TRACK => {
                    let mode = args_.case(0, 3).and_then(|v| {
                        super::types::TrackingMode::of(v).ok_or(_rt::Errno::EBADMSG)
                    })?;
                    match self.0.track(mode) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
                GOTO => {
                    let lon = args_.f64(0)?;
                    let lat = args_.f64(8)?;
                    match self.0.goto(lon, lat) {
                        Ok(()) => 0,
                        Err(e) => e as i32,
                    }
                }
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
