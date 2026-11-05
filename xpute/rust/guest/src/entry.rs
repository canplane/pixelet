// xpute-guest/entry.rs

//! The one call a guest runs inside, `run`, and what its host's form hands
//! it there. The floor names the entry it calls, as a C runtime names `main`:
//! a module exports `run` (`wasm`), a native library's thread loops on it
//! (xpute-host `native`), and the program implements it.
//!
//! Every host holds the guest to one contract. The guest runs until it has
//! nothing to do; a host may cut it at a slice's end, the quota, where the
//! guest's tasks yield (`sched::edge`). It returns when it wants to be woken:
//! 0 for the next moment its host's scheduling policy admits, `n` ms, or
//! `NO_WAKE` for only when something comes. A kick wakes it early: a submission, a completion, a call. A page
//! runs the guest on its one thread in turns its strobe grants, each cut at
//! the turn's quota (@xpute/host `Strobe`); a native host runs it on its own
//! thread, at the strobe's kick where it answered 0, and asleep until its
//! time or a kick otherwise (xpute-host native/thread.rs).

use crate::mem::heap::HeapStat;
use crate::stop::PanicHook;

/// What every guest is handed, whichever form it is built in. Asked for as a
/// function (`Entry`).
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The bytes of the core memory, which never grows.
    pub given: u32,
    /// Where the guest's own ranges may begin: in a module where the linker
    /// stopped; in a native library, the end of its config's front.
    pub data_end: u32,
    pub clock: fn() -> f64,
    pub hostcall: crate::hostcall::Hostcall,
    pub heap_stat: fn() -> HeapStat,
    /// Where a form whose guest shares its process with the host says why
    /// the guest stopped, and ends its thread (`stop::install`); none where
    /// the host outlives the guest apart from it (a page).
    pub stop_hook: Option<PanicHook>,
    /// Whether the GPU's host takes a frame now without blocking (xpute-gpu
    /// `admit`): a native host's own count, or a page's as its device's
    /// block tells it; none where the guest draws nothing.
    pub admit: Option<fn() -> bool>,
    /// What runs the stream the guest has just published at once, on the
    /// guest's thread, rung as END is stored (`ipc::stream`, xpute-gpu
    /// `host::native::publish`); none where the host runs it as the turn
    /// returns (a page).
    pub publish: Option<fn()>,
}

/// A turn of `quota_ms`, answering when the guest wants the next, in ms
/// (0 for the next turn, `sched::wake::NO_WAKE` for none).
pub type Entry = fn(quota_ms: f64, boot: fn() -> BootInfo) -> f64;

/// What a form's export runs: `entry`'s turn, the clock and the hostcall
/// installed from `boot` on the first.
pub fn run(quota_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !crate::clock::is_set() {
        let b = boot();
        crate::clock::set(b.clock);
        crate::hostcall::set(b.hostcall);
        crate::ipc::stream::on_publish(b.publish);
    }
    entry(quota_ms, boot)
}
