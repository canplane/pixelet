// rust/client/core/src/sys/machine.spec.rs
//
// GENERATED from spec/client/core/sys/machine.json

//! The machine its JSON declares (xpute-core machine.rs), which a build
//! script includes too as it links the module.

use xpute_core::machine::{Machine, Stream};

pub const MACHINE: Machine = Machine {
    mem_bytes: &[1073741824, 536870912],
    region_bytes: 67108864,
    min_heap_bytes: 268435456,
    stack_bytes: 1048576,
    thread_stack_bytes: 8388608,
    slot_bytes: 4096,
    sq_cap: 512,
    cq_cap: 1024,
    turn_posts: 174,
    fs: true,
    stream: Some(Stream { bytes: 4194304 }),
};

// The runtime's MACHINE places the front's parts at these same numbers.
const _: () = assert!(MACHINE.head_at() == 1048576);
const _: () = assert!(MACHINE.sq_at() == 1052672);
const _: () = assert!(MACHINE.cq_at() == 1064960);
const _: () = assert!(MACHINE.sq_slots_at() == 1085440);
const _: () = assert!(MACHINE.cq_slots_at() == 3182592);
const _: () = assert!(MACHINE.stream_at() == 7376896);

// What the core asks of its GPU: the power preference, and the features
// the device must have and those it takes where the adapter has them.
pub const GPU_ASK: (&str, &[&str], &[&str]) = (
    "high-performance",
    &["depth32float-stencil8"],
    &["timestamp-query"],
);
