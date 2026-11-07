// xpute-core/call.rs

//! The runtime's `call`: a call's frame at an offset, answered before it
//! returns, a value or -errno; ENOSYS until installed, as in native tests.

use core::cell::Cell;

use xpute_kit::status::errno::Errno;

/// The runtime's answer to a call's frame at an offset of the core memory.
pub type Call = fn(at: u32) -> i32;

thread_local! {
    static CALL: Cell<Option<Call>> = const { Cell::new(None) };
}

pub fn set(call: Call) {
    CALL.with(|c| c.set(Some(call)));
}

pub fn is_set() -> bool {
    CALL.with(Cell::get).is_some()
}

/// `at` is the frame's offset in the core memory.
pub fn call(at: u32) -> i32 {
    CALL.with(Cell::get).map_or(Errno::ENOSYS as i32, |f| f(at))
}

/// A runtime's answer as one value: a count, or the errno it refused with. A code
/// no errno names is the runtime breaking the protocol, EPROTO.
pub fn answer_of(res: i32) -> Result<u32, Errno> {
    u32::try_from(res).map_err(|_| Errno::of(res).unwrap_or(Errno::EPROTO))
}
