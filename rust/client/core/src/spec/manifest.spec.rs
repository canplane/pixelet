// pxl-core/spec/manifest.spec.rs
//
// GENERATED from spec/client/core/sys/manifest.json (xpute-gen data.rs)
//
// The core's manifest (xpute link/spec/manifest.schema.json): its memory, its fixed ranges and
// the heap it counts on, and its stacks.

/// `bytes`: the core's memory, 512 MiB on every platform, the limit chosen for it. The
/// page's worker grows the module's memory to it before the core's first entry, and the
/// core never grows it (sys/heap.rs), so a full heap stops the core: the stack, the
/// module's data, the fixed ranges, then the heap.
///
/// `fixed_bytes`: the fixed ranges past the module's data, `.fixed` (link/layout.rs), 147.63 MiB, which
/// the compiler holds the ranges' sum under.
///
/// `min_heap_bytes`: the heap the core counts on past the fixed ranges, 256 MiB, which the
/// boot checks the memory it is given leaves (sys/heap.rs). A whole file read and a fetched body are capped by it,
/// and the caps of the heap's bounded flows, added up in the buddy's blocks, are held under
/// it at compile time (link/layout.rs `HEAP_FLOWS_BYTES`).
pub mod mem {
    pub const BYTES: u32 = 536870912;
    pub const FIXED_BYTES: u32 = 154800128;
    pub const MIN_HEAP_BYTES: u32 = 268435456;
}

/// `module_bytes`: the stack, at the bottom of the memory, first so that an overflow
/// goes below zero, wraps as an unsigned 32-bit address and lands outside the memory,
/// which traps. With the stack after the data it would write over `.data` silently
/// instead. There is no MMU here and so no guard page: the wraparound is the whole of
/// the protection.
///
/// `thread_bytes`: the core's thread's stack on a device, where the core is a library
/// on a thread of its own: a main thread's 8 MiB, since React Native's JavaScript thread
/// has 1 MiB and wgpu's depth has no bound to state.
pub mod stack {
    pub const MODULE_BYTES: u32 = 1048576;
    pub const THREAD_BYTES: u32 = 8388608;
}
