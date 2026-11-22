// xpute-guest/abi/life.rs
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:life/life` (frame.json5 `NAMED`).
pub const HANDLE: _rt::Handle = 0x46edb9;

/// `hidden`
pub const HIDDEN: _rt::Method = 0;
/// `shown`
pub const SHOWN: _rt::Method = 1;
/// `attached`
pub const ATTACHED: _rt::Method = 2;

pub trait Guest {
    fn hidden(&self) -> Result<(), _rt::XputeError>;
    fn shown(&self) -> Result<(), _rt::XputeError>;
    fn attached(&self) -> Result<(), _rt::XputeError>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, _: _rt::Lift, ret_: &mut _rt::Lower) -> i32 {
        let run = || -> Result<i32, _rt::XputeError> {
            Ok(match m_ {
                HIDDEN => {
                    let () = self.0.hidden()?;
                    0
                }
                SHOWN => {
                    let () = self.0.shown()?;
                    0
                }
                ATTACHED => {
                    let () = self.0.attached()?;
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
    pub(super) use crate::link;
    pub(super) use std::rc::Rc;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_kit::status::error::XputeError;
    pub(super) use xpute_link::{Handle, Lift, Lower, Method, Object, Tag, store_err};
}
