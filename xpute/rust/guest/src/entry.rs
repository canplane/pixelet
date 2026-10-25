// xpute-guest/entry.rs

//! The one call a guest runs inside, `intr`, and what its host's form hands
//! it there. The floor names the entry it calls, as a C runtime names `main`
//! and `_start` and Windows `DllMain`: a module exports `intr` (`wasm`), a
//! library linked into a native program `xpute_intr` (xpute-host `native`),
//! and the program implements it, never renames it.

use crate::mem::heap::HeapStat;

/// What every guest is handed, whichever form it is built in: the memory it
/// was given, where its own ranges may begin, the host's clock and its
/// heap's reading. Asked for as a function (`Entry`), so a guest that reads
/// it once, as it boots, pays for nothing on its other turns.
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The bytes of the core memory, which never grows.
    pub given: u32,
    /// Where the form's own data ends and the guest's ranges may begin: in a
    /// module where the linker stopped (`wasm::data_end`); in a native
    /// library, whose statics are the process's, the end of the front its
    /// plan keeps at the memory's bottom.
    pub data_end: u32,
    /// The host's clock, in milliseconds (`clock`).
    pub clock: fn() -> f64,
    /// The allocator's reading.
    pub heap_stat: fn() -> HeapStat,
}

/// The guest's side of `intr`: a turn of `quota_ms`, with what it is handed,
/// and when it wants the next, in ms (`sched::quantum`: 0 for the next
/// frame, `NO_WAKE` for none).
pub type Entry = fn(quota_ms: f64, boot: fn() -> BootInfo) -> f64;
