// xpute-guest/hostcall.rs

//! The host's `hostcall`: a record at an offset, run before it returns, and
//! its answer, a value or -errno; ENOSYS until installed, as in native tests.

use core::cell::Cell;

use xpute_kit::status::errno::Errno;

/// The host's answer to a record at an offset of the core memory.
pub type Hostcall = fn(at: u32) -> i32;

thread_local! {
    static HOSTCALL: Cell<Option<Hostcall>> = const { Cell::new(None) };
}

pub fn set(hostcall: Hostcall) {
    HOSTCALL.with(|c| c.set(Some(hostcall)));
}

pub fn is_set() -> bool {
    HOSTCALL.with(Cell::get).is_some()
}

/// `at` is the record's offset in the core memory.
pub fn hostcall(at: u32) -> i32 {
    HOSTCALL
        .with(Cell::get)
        .map_or(Errno::ENOSYS as i32, |f| f(at))
}
