// rust/client/core/src/machine.spec.rs
//
// GENERATED from rust/client/core/machine.json

//! The machine its JSON declares (xpute-core machine.rs), which a build
//! script includes too as it links the module.

use xpute_core::machine::{Gpu, Machine};

pub const MACHINE: Machine = Machine {
    mem_bytes: &[1073741824, 536870912],
    region_bytes: 67108864,
    min_heap_bytes: 268435456,
    stack_bytes: 1048576,
    thread_stack_bytes: 8388608,
    slot_bytes: 4096,
    sq_cap: 512,
    cq_cap: 1024,
    turn_posts: 173,
    fs: true,
    gpu: Some(Gpu {
        bytes: 4194304,
        power: "high-performance",
        required: &["depth32float-stencil8"],
        optional: &["timestamp-query"],
    }),
};

// The runtime's MACHINE places the front's parts at these same numbers.
const _: () = assert!(MACHINE.head_at() == 1048576);
const _: () = assert!(MACHINE.sq_at() == 1052672);
const _: () = assert!(MACHINE.cq_at() == 1064960);
const _: () = assert!(MACHINE.sq_slots_at() == 1085440);
const _: () = assert!(MACHINE.cq_slots_at() == 3182592);
const _: () = assert!(MACHINE.gpu_at() == 7376896);
