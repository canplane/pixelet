// pxl-wasm/lib.rs

//! The one WebAssembly module: pxl-kernel behind a directory and a doorbell.
//!
//! No wasm-bindgen. The central scheduler gives the module its memory, and
//! the kernel lays out everything inside it on its first turn
//! (pxl_kernel::sys::mem); the shell reads the directory once, at an address both
//! sides know, and everything after crosses in that memory
//! (pxl_kernel::door). What is here is the bracket: what the linker and the
//! page decide — where the module's own data ends, how much memory it was
//! given, the allocator over it, the clock — and `interrupt`, the export the
//! shell rings.

use pxl_kernel::door::layout::heap_at;
use pxl_kernel::door::Bracket;
use xpute_guest::mem::heap::{Bounds, Heap, HeapStat};
use xpute_guest::module::{data_end, given, now, stack_high};

/// Where the module's heap lies: from the page past the last fixed range to
/// the end of the memory, ascending. Everything before it is settled before
/// it — the stack, the rings, the linker's data, and `.fixed`.
struct ModuleBounds;

impl Bounds for ModuleBounds {
    fn base() -> usize {
        heap_at(data_end()) as usize
    }
    fn end() -> usize {
        given() as usize
    }
}

/// The module's one allocator (xpute_guest::mem::heap).
#[global_allocator]
static ALLOC: Heap<ModuleBounds> = Heap::new();

/// The allocator's reading, for the profiler and the heap's row.
fn heap_stat() -> HeapStat {
    xpute_guest::mem::heap::heap_stat(&ALLOC)
}

/// The browser paces a page's frames to what the GPU finishes, and taking a
/// canvas's texture never blocks: every frame is admitted.
fn admit_frame() -> bool {
    true
}

/// What this module gives the kernel: the memory the page grew it to, the
/// linker's end of its data, the page's clock, the allocator and the frames
/// it admits.
fn bracket() -> Bracket {
    let high = stack_high();
    xpute_kit::ensure!(high == pxl_kernel::sys::mem::spec::STACK_BYTES, ENOTRECOVERABLE, high);
    let given = given();
    Bracket {
        given: given.min(u32::MAX as u64) as u32,
        data_end: data_end(),
        clock: now,
        heap_stat,
        admit_frame,
        last_words: None,
    }
}

/// The doorbell (pxl_kernel::door::interrupt): a turn with the time it may
/// take, and when the kernel wants the next.
#[no_mangle]
pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
    pxl_kernel::door::interrupt(quota_ms, bracket)
}
