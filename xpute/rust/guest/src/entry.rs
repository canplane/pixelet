// xpute-guest/entry.rs

//! The one call a guest runs inside, `intr`, and what its host's form hands
//! it there. The floor names the entry it calls, as a C runtime names `main`:
//! a module exports `intr` (`wasm`), a native library `xpute_intr`
//! (xpute-host `native`), and the program implements it.

use crate::mem::heap::HeapStat;

/// What every guest is handed, whichever form it is built in. Asked for as a
/// function (`Entry`), so a guest that reads it once pays nothing after.
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The bytes of the core memory, which never grows.
    pub given: u32,
    /// Where the guest's own ranges may begin: in a module where the linker
    /// stopped; in a native library, the end of its plan's front.
    pub data_end: u32,
    pub clock: fn() -> f64,
    pub heap_stat: fn() -> HeapStat,
}

/// A turn of `quota_ms`, answering when the guest wants the next, in ms
/// (`sched::quantum`: 0 for the next frame, `NO_WAKE` for none).
pub type Entry = fn(quota_ms: f64, boot: fn() -> BootInfo) -> f64;

/// What a form's export runs: `entry`'s turn, the clock installed from
/// `boot` on the first.
pub fn intr(quota_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !crate::clock::is_set() {
        crate::clock::set(boot().clock);
    }
    entry(quota_ms, boot)
}
