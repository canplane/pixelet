// xpute-core/call.rs

//! The runtime's synchronous `call`. The core writes a call's frame into the
//! core memory and passes the frame's offset; the runtime answers before
//! `call` returns, with a value or a negative errno. Until a runtime installs
//! its `call`, every call answers ENOSYS, which native tests run with.

use core::cell::Cell;

use xpute_kit::status::errno::Errno;

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

pub fn call(at: u32) -> i32 {
    CALL.with(Cell::get).map_or(Errno::ENOSYS as i32, |f| f(at))
}

/// Reads a runtime's answer as a count, or as the errno it refused with. A
/// negative code that names no errno means the runtime broke the protocol,
/// and reads as EPROTO.
pub fn answer_of(res: i32) -> Result<u32, Errno> {
    u32::try_from(res).map_err(|_| Errno::of(res).unwrap_or(Errno::EPROTO))
}
