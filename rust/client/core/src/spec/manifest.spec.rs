// pxl-core/spec/manifest.spec.rs
//
// GENERATED from spec/client/core/sys/manifest.json5 (xpute-gen data.rs)
//
// The core's manifest: its memory, its fixed ranges and
// the heap it counts on, and its stacks.

/// `sz`: the core's memory, 512 MiB on every platform, the limit chosen for it:
/// on a page the module's memory, which its link starts at this size and holds to it
/// (build.rs), and on a device a block the core reserves of its process at it (sys/heap.rs). Nothing
/// grows it, so a full heap stops the core: the stack, the module's data, the fixed ranges, then the
/// heap (sys/heap.rs).
///
/// `fixed_sz`: the fixed ranges, `.fixed` (link/layout.rs), 147.68 MiB, which the compiler holds
/// the ranges' sum under. In a module it begins on the page past the module's data, and on a
/// device at the block's base; the heap begins on the page past it (sys/heap.rs).
///
/// `min_heap_sz`: the heap the core counts on past the fixed ranges, 256 MiB, which the
/// boot checks the memory leaves (link/layout.rs `install`). A whole file read and a fetched body are capped by it,
/// and the caps of the heap's bounded flows, added up in the buddy's blocks, are held under
/// it at compile time (link/layout.rs `HEAP_FLOWS_SZ`).
pub mod mem {
    pub const SZ: u32 = 536870912;
    pub const FIXED_SZ: u32 = 154849280;
    pub const MIN_HEAP_SZ: u32 = 268435456;
}

/// `module_sz`: the stack, at the bottom of the memory, first so that an overflow
/// goes below zero, wraps as an unsigned 32-bit address and lands outside the memory,
/// which traps. With the stack after the data it would write over `.data` silently
/// instead. There is no MMU here and so no guard page: the wraparound is the whole of
/// the protection.
///
/// `thread_sz`: the core's thread's stack on a device, where the core is a library
/// on a thread of its own: a main thread's 8 MiB, since React Native's JavaScript thread
/// has 1 MiB and wgpu's depth has no bound to state.
pub mod stack {
    pub const MODULE_SZ: u32 = 1048576;
    pub const THREAD_SZ: u32 = 8388608;
}
