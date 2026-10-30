// pxl-wasm/lib.rs

//! The one WebAssembly module: pxl-kernel behind a directory and a doorbell.
//!
//! No wasm-bindgen. The central scheduler gives the module its memory, and
//! the kernel lays out everything inside it on its first turn
//! (pxl_kernel::sys::mem); the shell reads the directory once, at an address both
//! sides know, and everything after crosses in that memory
//! (pxl_kernel::door). The module's form is xpute's (xpute_guest::wasm): the
//! allocator over the memory, what the module reads of itself, and `run`,
//! the export the shell rings. What is here is pixelet's: its numbers, where
//! its heap begins and the kernel as the entry.

use pxl_kernel::door;
use pxl_kernel::door::layout::heap_at;
use pxl_kernel::sys::mem::spec::STACK_BYTES;
use xpute_gpu::admit;
use xpute_guest::entry::BootInfo;
use xpute_guest::wasm::Config;

// The heap from the page past the last fixed range to the end of the memory:
// everything before it is settled before it — the stack, the rings, the
// linker's data, and `.fixed` (door/layout.rs `heap_at`).
xpute_guest::wasm_abi!(
    Config {
        heap_at,
        stack_bytes: STACK_BYTES
    },
    kernel
);

/// The kernel's turn (pxl_kernel::door::run). A page outlives the kernel,
/// so it has no last words to say; the browser paces its frames, so every
/// one is admitted (xpute_gpu::admit).
fn kernel(quota_ms: f64, boot_info: fn() -> BootInfo) -> f64 {
    door::run(quota_ms, boot_info, None, admit::every)
}
