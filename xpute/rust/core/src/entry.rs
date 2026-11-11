// xpute-core/entry.rs

//! This is the one function a core runs inside, `run`, and what the runtime
//! hands it there. xpute names this entry: a module exports `run` (`wasm`), a
//! native library's thread calls it in a loop (xpute-runtime `native`), and
//! the program implements it.
//!
//! A core's form is how it is built: a WebAssembly module or a native library.
//! Every runtime, whichever form the core is built in, holds the core to one
//! contract. In a turn the core runs until it has nothing left to do or its
//! deadline passes; at the deadline its tasks yield (`sched::turn`). It then
//! returns when it wants to be woken: 0 for the earliest moment the runtime's
//! scheduling policy allows, `n` for `n` ms from now, or `NO_WAKE` for only
//! when something arrives. The runtime asks for a turn earlier for each frame
//! it sends the core.
//!
//! A page runs the core on its one thread, in turns its scheduler grants and
//! cuts off at each turn's deadline (@xpute/runtime `Scheduler`). A native
//! runtime runs the core on a thread of its own, which begins a wanted turn at
//! a display frame (xpute-runtime native/thread.rs).

use core::cell::Cell;

use crate::mem::heap::HeapStat;
use crate::stop::PanicHook;

/// Every core is handed this, in whichever form it is built. A module's
/// values come from its linker and its page (`wasm`); a native library's from
/// its runtime and its config (xpute-runtime `native::Config`). An entry
/// receives it as a function to call (`Entry`).
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// The size of the core memory in bytes. The memory never grows.
    pub mem_sz: u32,
    /// How the core reaches its runtime (`host`).
    pub host: crate::host::Host,
    pub heap_stat: fn() -> HeapStat,
    /// The first offset at which the core may place its own ranges. In a
    /// module it is where the linker's data ends; in a native library it is
    /// the end of the front of the memory (`manifest`), or 0 for none.
    pub data_end: u32,
    pub now: fn() -> f64,
    /// Where the core shares its process with the runtime, this hook reports
    /// why the core stopped and ends its thread (`stop::install`). A page has
    /// none, since the page's runtime outlives the core on its own.
    pub stop_hook: Option<PanicHook>,
    /// Hands over the stream the core has just published; it is called as
    /// END is stored (`io::stream`).
    /// A page has none, since its runtime runs the stream when the turn
    /// returns.
    pub publish: Option<fn()>,
}

/// An entry runs one turn that may last `deadline_ms`, and answers in how many
/// ms the core wants the next: 0 for as soon as the scheduling policy allows,
/// `sched::wake::NO_WAKE` for none.
pub type Entry = fn(deadline_ms: f64, boot: fn() -> BootInfo) -> f64;

thread_local! {
    static BOOTED: Cell<bool> = const { Cell::new(false) };
}

/// A form's exported entry calls this on every turn. On the first turn it
/// installs the clock, the host and the publish hook from `boot`. It then runs
/// `entry`.
pub fn run(deadline_ms: f64, boot: fn() -> BootInfo, entry: Entry) -> f64 {
    if !BOOTED.with(|b| b.replace(true)) {
        let b = boot();
        crate::clock::set(b.now);
        crate::host::set(b.host);
        crate::io::stream::on_publish(b.publish);
    }
    entry(deadline_ms, boot)
}

/// Defines a core of `manifest` (its `MANIFEST`) in the form its target takes.
/// Its fixed ranges are `fixed_bytes` long, and `entry` runs its turns
/// (`entry::Entry`) once the first turn has opened the front the runtime
/// wrote (io/door.rs `boot`). As a WebAssembly module the core is `wasm_abi!` over the
/// manifest, with the heap past the fixed ranges and the stack it was linked
/// with. On any other target it is xpute-runtime's `native_core!`, so a
/// program's crate depends on xpute-runtime for every target but wasm32.
///
/// ```ignore
/// xpute_core::core!(MANIFEST, FIXED_BYTES, door::run);
/// ```
#[macro_export]
macro_rules! core {
    (@wasm $manifest:expr, $fixed_bytes:expr, $entry:expr) => {
        #[cfg(target_arch = "wasm32")]
        mod xpute_core_manifest {
            #[allow(unused_imports)]
            use super::*;

            pub(super) fn xpute_turn(deadline_ms: f64, boot: fn() -> $crate::entry::BootInfo) -> f64 {
                $crate::io::door::boot(&$manifest);
                ($entry)(deadline_ms, boot)
            }
        }

        #[cfg(target_arch = "wasm32")]
        $crate::wasm_abi!(
            $crate::wasm::Config {
                heap_at: |data_end| $manifest.heap_at(data_end, $fixed_bytes),
                stack_bytes: $manifest.stack_bytes,
            },
            xpute_core_manifest::xpute_turn
        );
    };
    ($manifest:expr, $fixed_bytes:expr, $entry:expr $(,)?) => {
        const _: () = $manifest.check($fixed_bytes);
        const _: $crate::entry::Entry = $entry;

        $crate::core!(@wasm $manifest, $fixed_bytes, $entry);

        #[cfg(not(target_arch = "wasm32"))]
        ::xpute_runtime::native_core!($manifest, $fixed_bytes, $entry);
    };
}
