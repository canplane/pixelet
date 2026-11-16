// pxl-gpu-guest/abi/surface.rs
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

/// The interface's handle, named by `pxl:gpu/surface` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x76d49a;

/// `[method]surface.resize`
pub const SURFACE_RESIZE: _rt::Method = 0;

pub trait GuestSurface {
    fn resize(&self, width: u32, height: u32, dpr: f64) -> Result<(), _rt::Errno>;
}

struct _SurfaceObject(_rt::Rc<dyn GuestSurface>);

impl _rt::Object for _SurfaceObject {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: _rt::Lift, _: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::Errno> {
            Ok(match m_ {
                SURFACE_RESIZE => {
                    let width = args_.u32(0)?;
                    let height = args_.u32(4)?;
                    let dpr = args_.f64(8)?;
                    match self.0.resize(width, height, dpr) {
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

/// The object over `implementation`, for an opener or a function to answer.
pub fn surface(implementation: _rt::Rc<dyn GuestSurface>) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_SurfaceObject(implementation))
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag};
}
