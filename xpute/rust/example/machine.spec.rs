// rust/example/machine.spec.rs
//
// GENERATED from rust/example/machine.json

//! The machine its JSON declares (xpute-guest machine.rs), which a build
//! script includes too as it links the module.

use xpute_guest::machine::Machine;

pub const MACHINE: Machine = Machine {
    mem_bytes: &[2097152],
    region_bytes: 2097152,
    min_heap_bytes: 65536,
    stack_bytes: 65536,
    thread_stack_bytes: 1048576,
    slot_bytes: 256,
    sq_cap: 8,
    cq_cap: 16,
    turn_posts: 1,
    gpu: None,
};

// The host's MACHINE places the front's parts at these same numbers.
const _: () = assert!(MACHINE.head_at() == 65536);
const _: () = assert!(MACHINE.sq_at() == 69632);
const _: () = assert!(MACHINE.cq_at() == 73728);
const _: () = assert!(MACHINE.sq_slots_at() == 77824);
const _: () = assert!(MACHINE.cq_slots_at() == 79872);
const _: () = assert!(MACHINE.gpu_at() == 0);
