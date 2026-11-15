// xpute-host/abi/life.rs
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

pub fn hidden(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, HIDDEN, _rt::Packet::Args(None));
    _rt::answer(r)
}

pub fn shown(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, SHOWN, _rt::Packet::Args(None));
    _rt::answer(r)
}

pub fn attached(side_: &_rt::Side) -> Result<(), _rt::Errno> {
    let (r, _) = side_.call(HANDLE, ATTACHED, _rt::Packet::Args(None));
    _rt::answer(r)
}

mod _rt {
    pub(super) use xpute_kit::status::errno::Errno;
    pub(super) use xpute_link::{Handle, Method, Packet, Side, answer};
}

/// The interfaces the world imports, each `(name, handle)`, which are
/// mounted before the guest boots: one with functions of its own under its
/// handle, one of resources alone by an opener at a prefix.
pub const IMPORTS: &[(&str, xpute_link::Handle)] = &[];

/// The methods of the interfaces the world imports that answer at once,
/// each `(handle, method)`, a function without WIT's `async`, which a
/// runtime refuses to carry to a UI on another thread.
pub const CALLS: &[(xpute_link::Handle, xpute_link::Method)] = &[];
