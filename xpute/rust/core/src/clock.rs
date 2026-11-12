// xpute-core/clock.rs

//! The runtime's clock (wit/clock.wit), called in an entry.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi;
use crate::mem;

/// The time in ms, which never runs backward; only differences mean anything.
pub fn now() -> f64 {
    read(0)
}

/// The wall clock, in Unix ms.
pub fn wall() -> f64 {
    read(1)
}

/// The `k`th of the two f64 the clock writes into the core's memory.
fn read(k: u32) -> f64 {
    thread_local! {
        static AT: u32 = mem::off(Box::leak(Box::new([0u64; 2])).as_ptr().cast());
    }
    let at = AT.with(|a| *a);
    abi::clock::now(at).or_bug(Errno::EPROTO);
    // SAFETY: the clock's two words, which only `read` lends.
    let b = unsafe { mem::bytes(at + 8 * k, 8) };
    f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}
