// rust/client/core/src/sys/manifest.spec.rs
//
// GENERATED from spec/client/core/sys/manifest.json

//! The manifest its JSON declares, which a build script includes too as it
//! links the module.

/// The core's memory, whole before its first entry and never grown.
pub const MEM_BYTES: u32 = 536870912;
/// The program's fixed ranges, past the module's data.
pub const FIXED_BYTES: u32 = 154800128;
/// The heap the core counts on, past the fixed ranges.
pub const MIN_HEAP_BYTES: u32 = 268435456;
/// A module's stack, first in its memory.
pub const STACK_BYTES: u32 = 1048576;
/// A native core's thread's stack.
pub const THREAD_STACK_BYTES: u32 = 8388608;
/// The stream's room.
pub const STREAM_BYTES: u32 = 4194304;

// What the core asks of its GPU: the power preference, and the features
// the device must have and those it takes where the adapter has them.
pub const GPU_ASK: (&str, &[&str], &[&str]) = (
    "high-performance",
    &["depth32float-stencil8"],
    &["timestamp-query"],
);
