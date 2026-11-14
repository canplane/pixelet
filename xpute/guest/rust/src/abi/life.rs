// xpute-guest/abi/life.rs
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

/// The interface's handle, named by `xpute:life/life` (frame.json `NAMED`).
pub const HANDLE: _rt::Handle = 0x46edb9;

/// `hidden`
pub const HIDDEN: _rt::Method = 0;
/// `shown`
pub const SHOWN: _rt::Method = 1;

pub trait Guest {
    fn hidden(&self) -> Result<(), _rt::Errno>;
    fn shown(&self) -> Result<(), _rt::Errno>;
}

struct _Object(_rt::Rc<dyn Guest>);

impl _rt::Object for _Object {
    fn call(&self, m_: _rt::Method, _: _rt::Tag, _: Option<&[u8]>) -> _rt::Reply {
        let run = || -> Result<_rt::Reply, _rt::Errno> {
            Ok(match m_ {
                HIDDEN => match self.0.hidden() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                SHOWN => match self.0.shown() {
                    Ok(()) => _rt::Reply::from(0),
                    Err(e) => _rt::Reply::from(e as i32),
                },
                _ => _rt::Reply::from(_rt::Errno::ENOSYS as i32),
            })
        };
        run().unwrap_or_else(|e| _rt::Reply::from(e as i32))
    }
}

/// Serves `implementation` under the interface's handle.
pub fn mount(implementation: _rt::Rc<dyn Guest>) {
    _rt::link::mount_named(HANDLE, _rt::Rc::new(_Object(implementation)));
}

mod _rt {
    pub use crate::link;
    pub use std::rc::Rc;
    pub use xpute_kit::status::errno::Errno;
    pub use xpute_link::{Handle, Method, Object, Reply, Tag};
}
