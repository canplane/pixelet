// xpute-core/entry.rs

//! This is the one function a core runs inside, `run`, and what the runtime
//! hands it there. xpute names this entry as a C runtime names `main`: a
//! module exports `run` (`wasm`), a native library's thread calls it in a loop
//! (xpute-runtime `native`), and the program implements it.
//!
//! A core's form is how it is built: a WebAssembly module or a native library.
//! Every runtime, whichever form the core is built in, holds the core to one
//! contract. In a turn the core runs until it has nothing left to do or its
//! deadline passes; at the deadline its tasks yield (`sched::edge`). It then
//! returns when it wants to be woken: 0 for the earliest moment the runtime's
//! scheduling policy allows, `n` for `n` ms from now, or `NO_WAKE` for only
//! when something arrives. A kick wakes it earlier: the runtime kicks it for
//! each submission, completion and call it sends, and after a read of the
//! completion ring that took frames, since that read gave back room a task may
//! be waiting for.
//!
//! A page runs the core on its one thread, in turns its scheduler grants and
//! cuts off at each turn's deadline (@xpute/runtime `Scheduler`). A native
//! runtime runs the core on a thread of its own, which sleeps until a kick.
//! The same scheduler sends that kick at the moment its policy allows for
//! the wake the core asked for (xpute-runtime native/thread.rs).

use crate::mem::heap::HeapStat;
use crate::stop::PanicHook;

/// Every core is handed this, in whichever form it is built: what its runtime
/// measures and installs, and what depends on its form (`Form`). An entry
/// receives it as a function to call (`Entry`).
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The size of the core memory in bytes. The memory never grows.
    pub mem_sz: u32,
    pub call: crate::call::Call,
    pub heap_stat: fn() -> HeapStat,
    pub form: Form,
}

/// This is what depends on the core's form. A module's values come from its
/// linker and its page (`wasm`); a native library's come from its config
/// (xpute-runtime `native::Config`).
#[derive(Clone, Copy)]
pub struct Form {
    /// The first offset at which the core may place its own ranges. In a
    /// module it is where the linker's data ends; in a native library it is
    /// the end of the front of the memory (`machine`), or 0 for none.
    pub data_end: u32,
    pub now: fn() -> f64,
    /// Where the core shares its process with the runtime, this hook reports
    /// why the core stopped and ends its thread (`stop::install`). A page has
    /// none, since the page's runtime outlives the core on its own.
    pub stop_hook: Option<PanicHook>,
    /// Whether the GPU's runtime takes a frame now without blocking
    /// (xpute-gpu `admit`). A native runtime answers from its own count of
    /// unfinished frames; a page answers from the count its device's block
    /// reports. A core that draws nothing has none.
    pub admit_frame: Option<fn() -> bool>,
    /// Hands over the stream the core has just published; it is called as
    /// END is stored (`ipc::stream`, xpute-gpu `runtime::native::publish`).
    /// A page has none, since its runtime runs the stream when the turn
    /// returns.
    pub publish: Option<fn()>,
}

/// An entry runs one turn that may last `deadline_ms`, and answers in how many
/// ms the core wants the next: 0 for as soon as the scheduling policy allows,
/// `sched::wake::NO_WAKE` for none.
pub type Entry = fn(deadline_ms: f64, boot: fn() -> BootInfo) -> f64;

/// A form's exported entry calls this on every turn. On the first turn it
/// installs the call and the publish hook from `boot`, and the form's clock
/// unless a runtime that keeps the core's time has already set one
/// (`clock::set`). It then runs `entry`.
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
