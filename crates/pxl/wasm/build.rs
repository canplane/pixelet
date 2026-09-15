// The module imports its memory: the central scheduler gives it one
// (@xpute/core/sys/memory.ts's reserve_memory), and the kernel lays out
// everything inside it (pxl_core::memory, over xpute_core::sys::memory).
// Declared no larger than its data needs and with no maximum, so whatever the
// central scheduler gives satisfies it.
fn main() {
    println!("cargo:rustc-link-arg=--import-memory");
}
