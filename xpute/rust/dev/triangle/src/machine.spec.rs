// rust/dev/triangle/src/machine.spec.rs
//
// GENERATED from rust/dev/triangle/machine.json

//! The machine its JSON declares (xpute-core machine.rs), which a build
//! script includes too as it links the module.

use xpute_core::machine::{Machine, Stream};

pub const MACHINE: Machine = Machine {
    mem_bytes: &[4194304],
    region_bytes: 4194304,
    min_heap_bytes: 65536,
    stack_bytes: 65536,
    thread_stack_bytes: 1048576,
    slot_bytes: 256,
    sq_cap: 8,
    cq_cap: 16,
    turn_posts: 1,
    fs: false,
    stream: Some(Stream { bytes: 65536 }),
};

// The runtime's MACHINE places the front's parts at these same numbers.
const _: () = assert!(MACHINE.head_at() == 65536);
const _: () = assert!(MACHINE.sq_at() == 69632);
const _: () = assert!(MACHINE.cq_at() == 73728);
const _: () = assert!(MACHINE.sq_slots_at() == 77824);
const _: () = assert!(MACHINE.cq_slots_at() == 79872);
const _: () = assert!(MACHINE.stream_at() == 83968);

// What the core asks of its GPU: the power preference, and the features
// the device must have and those it takes where the adapter has them.
pub const GPU_ASK: (&str, &[&str], &[&str]) = ("low-power", &[], &[]);
