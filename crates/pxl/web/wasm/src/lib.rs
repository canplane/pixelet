// pxl-wasm/lib.rs

//! The one WebAssembly module: pxl-kernel behind a boot block and a doorbell.
//!
//! No wasm-bindgen. The central scheduler gives the module its memory, and
//! the kernel lays out everything inside it on its first turn
//! (pxl_kernel::mem); the shell reads the boot block once, at an address both
//! sides know, and everything after crosses in that memory
//! (pxl_kernel::door). What is here is the bracket: what the linker and the
//! page decide — where the module's own data ends, how much memory it was
//! given, the allocator over it, the clock — and `interrupt`, the export the
//! shell rings.

use pxl_kernel::door::Bracket;
use pxl_kernel::mem::heap::{HeapBounds, KernelAlloc};
use pxl_kernel::mem::section;

extern "C" {
    /// Where the linker stopped: the end of `.rodata`, `.data` and `.bss`, and
    /// so where the kernel may begin placing its own ranges. wasm-ld defines
    /// it, and `--global-base` (build.rs) is what decides where the data it
    /// measures began.
    ///
    /// Not `__heap_base`, though that symbol sits eight bytes further on and
    /// would do. The linker calls it that because a C runtime starts its
    /// `sbrk` there; this kernel's heap is past the fixed ranges and past the
    /// terrain, so borrowing the name would leave `heap` meaning two things
    /// two hundred megabytes apart.
    #[allow(non_upper_case_globals)]
    static __data_end: u8;
    /// The top of the stack: `-z stack-size` past its start, which
    /// `--stack-first` puts at the bottom of the memory. wasm-ld defines it.
    #[allow(non_upper_case_globals)]
    static __stack_high: u8;
}

/// Where the linker stopped, as an address.
fn data_end() -> u32 {
    core::ptr::addr_of!(__data_end) as u32
}

/// Where the module's heap lies: from the page past the last fixed range to
/// the end of the memory, ascending. Everything before it is settled before
/// it — the stack, the rings, the linker's data, and `.fixed`.
struct ModuleBounds;

impl HeapBounds for ModuleBounds {
    fn base() -> usize {
        section::heap_at(data_end()) as usize
    }
    fn end() -> usize {
        core::arch::wasm32::memory_size(0) * (1 << 16)
    }
}

/// The module's one allocator (pxl_kernel::mem::heap).
#[global_allocator]
static ALLOC: KernelAlloc<ModuleBounds> = KernelAlloc::new();

// `wasm_import_module` is what makes this an import, and it is load-bearing
// rather than documentation. wasm-ld used to be handed `--allow-undefined`,
// under which any symbol nothing defined became an import of the module named
// `env` — which is the module the shell instantiates with
// (apps/web/src/kernel/module.ts). Rust stopped passing that flag in 1.96, so
// an extern block that says nothing is a link error.
#[link(wasm_import_module = "env")]
extern "C" {
    /// The client's clock, in milliseconds: what a tick's budget is spent against.
    #[link_name = "now"]
    fn host_now() -> f64;
}

/// The allocator's reading, for the ledger and the heap's row.
fn heap_stat() -> pxl_kernel::host::HeapStat {
    pxl_kernel::mem::heap::heap_stat(&ALLOC)
}

fn now() -> f64 {
    // SAFETY: an import with no arguments.
    unsafe { host_now() }
}

/// What this module gives the kernel: the memory the page grew it to, the
/// linker's end of its data, the page's clock and the allocator.
fn bracket() -> Bracket {
    let stack_high = core::ptr::addr_of!(__stack_high) as u32;
    xpute_core::ensure!(stack_high == pxl_kernel::mem::spec::STACK_BYTES, ENOTRECOVERABLE, stack_high);
    let given = core::arch::wasm32::memory_size(0) as u64 * (1 << 16);
    Bracket {
        given: given.min(u32::MAX as u64) as u32,
        data_end: data_end(),
        clock: now,
        heap_stat,
        last_words: None,
    }
}

/// The doorbell (pxl_kernel::door::interrupt): a turn with the time it may
/// take, and when the kernel wants the next.
#[no_mangle]
pub extern "C" fn interrupt(quota_ms: f64) -> f64 {
    pxl_kernel::door::interrupt(quota_ms, bracket)
}
