// xpute-native-runtime/abi/life.rs
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

pub fn hidden() -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(HANDLE, HIDDEN, None);
    _rt::answer(r)
}

pub fn shown() -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(HANDLE, SHOWN, None);
    _rt::answer(r)
}

pub fn attached() -> Result<(), _rt::Errno> {
    let (r, _) = _rt::link::call(HANDLE, ATTACHED, None);
    _rt::answer(r)
}

mod _rt {
    pub(super) use crate::link;
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{answer, Handle, Method};
}

/// The methods of the named interfaces the host serves that answer
/// later, each `(handle, method)`, as WIT's `async func` marks them.
pub const LATER: &[(xpute_link::Handle, xpute_link::Method)] = &[];
