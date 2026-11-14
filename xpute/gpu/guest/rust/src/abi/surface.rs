// xpute-gpu-guest/abi/surface.rs
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

/// `[method]surface.resize`
pub const SURFACE_RESIZE: _rt::Method = 0;

pub trait GuestSurface {
    fn resize(&self, width: u32, height: u32, dpr: f64) -> Result<(), _rt::Errno>;
}

struct _SurfaceObject(_rt::Rc<dyn GuestSurface>);

impl _rt::Object for _SurfaceObject {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, args_: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                SURFACE_RESIZE => {
                    let r_ = _rt::Lift::new(args_.ok_or(_rt::Errno::EBADMSG)?);
                    let width = r_.u32(0)?;
                    let height = r_.u32(4)?;
                    let dpr = r_.f64(8)?;
                    match self.0.resize(width, height, dpr) {
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

/// The object over `implementation`, for an opener or a function to answer.
pub fn surface(implementation: _rt::Rc<dyn GuestSurface>) -> _rt::Rc<dyn _rt::Object> {
    _rt::Rc::new(_SurfaceObject(implementation))
}

mod _rt {
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Lift, Method, Object, Reply, Tag};
}
