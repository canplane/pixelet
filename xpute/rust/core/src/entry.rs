// xpute-core/entry.rs

//! The one call a core runs inside, `run`, and what its runtime's form hands
//! it there. xpute names the entry it calls, as a C runtime names `main`:
//! a module exports `run` (`wasm`), a native library's thread loops on it
//! (xpute-runtime `native`), and the program implements it.
//!
//! Every runtime holds the core to one contract. The core runs until it has
//! nothing to do or its deadline passes, where its tasks yield
//! (`sched::edge`). It returns when it wants to be woken:
//! 0 for the next moment its runtime's scheduling policy admits, `n` ms, or
//! `NO_WAKE` for only when something comes. A kick wakes it early: a submission, a completion, a call. A page
//! runs the core on its one thread in turns its scheduler grants, each cut at
//! the turn's deadline (@xpute/runtime `Scheduler`); a native runtime runs it on its own
//! thread, asleep until a kick, which the same scheduler gives at the moment its
//! policy admits for the wake the core answered (xpute-runtime
//! native/thread.rs).

use crate::mem::heap::HeapStat;
use crate::stop::PanicHook;

/// What every core is handed, whichever form it is built in: what its runtime
/// measures and installs, and what its form says (`Form`). Asked for as a
/// function (`Entry`).
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The bytes of the core memory, which never grows.
    pub given: u32,
    pub call: crate::call::Call,
    pub heap_stat: fn() -> HeapStat,
    pub form: Form,
}

/// What a core's form says of it: a module's as its linker and its page give
/// it (`wasm`), a native library's as its config does (xpute-runtime
/// `native::Config`).
#[derive(Clone, Copy)]
pub struct Form {
    /// Where the core's own ranges may begin: in a module where the linker
    /// stopped; in a native library, the end of its config's front.
    pub data_end: u32,
    pub now: fn() -> f64,
    /// Where a form whose core shares its process with the runtime says why
    /// the core stopped, and ends its thread (`stop::install`); none where
    /// the runtime outlives the core apart from it (a page).
    pub stop_hook: Option<PanicHook>,
    /// Whether the GPU's runtime takes a frame now without blocking (xpute-gpu
    /// `admit`): a native runtime's own count, or a page's as its device's
    /// block tells it; none where the core draws nothing.
    pub admit_frame: Option<fn() -> bool>,
    /// What runs the stream the core has just published at once, on the
    /// core's thread, rung as END is stored (`ipc::stream`, xpute-gpu
    /// `runtime::native::publish`); none where the runtime runs it as the turn
    /// returns (a page).
    pub publish: Option<fn()>,
}

/// A turn of `deadline_ms`, answering when the core wants the next, in ms
/// (0 for the next turn, `sched::wake::NO_WAKE` for none).
pub type Entry = fn(deadline_ms: f64, boot: fn() -> BootInfo) -> f64;

/// What a form's export runs: `entry`'s turn, the call and the publish
/// installed from `boot` on the first, and its clock where none was set
/// before it, as a runtime that keeps the core's time sets one (`clock::set`).
pub fn run(deadline_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !crate::call::is_set() {
        let b = boot();
        if !crate::clock::is_set() {
            crate::clock::set(b.form.now);
        }
        crate::call::set(b.call);
        crate::ipc::stream::on_publish(b.form.publish);
    }
    entry(deadline_ms, boot)
}
