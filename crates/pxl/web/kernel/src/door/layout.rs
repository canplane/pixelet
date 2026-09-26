// pxl-kernel/door/layout.rs

//! The front of the memory, below where a bracket's own data begins
//! (GLOBAL_BASE): the three rings' descriptors and their slots' payloads, at
//! addresses both sides compute from mem.json rather than discover, so the
//! shell finds them from the boot block alone. The numbers are mem.json's,
//! the same file the map and the shell read (pxl-tool/bin/spec.rs writes
//! mem/mem.spec.rs).

use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes};

use crate::mem::spec::{BOOT_AT, GLOBAL_BASE, PAGE_BYTES, RING_CAPACITY as CAPACITY, RING_DESC_AT as DESC_AT, RING_SLOT_AT as SLOT_AT, STACK_BYTES};

/// Messages a ring holds.
pub const RING_CAPACITY: u32 = CAPACITY;

/// A ring slot's payload is a page: a packet is at most one, and anything
/// larger is bulk and crosses by address. The page is also the buddy's
/// smallest block, so a payload is one block wherever one has to come from.
pub const RING_SLOT_BYTES: u32 = PAGE_BYTES;

/// Where each ring's descriptors and payloads are, at addresses that are the
/// same in every build rather than wherever something handed them out.
pub const RING_DESC_AT: u32 = DESC_AT;
pub const RING_SLOT_AT: u32 = SLOT_AT;

/// The descriptors: a header of 8 words and `RING_CAPACITY` messages of 4,
/// rounded up to a page so each ring begins on one.
pub const RING_DESC_STRIDE: u32 = ((8 + RING_CAPACITY * 4) * 4).div_ceil(PAGE_BYTES) * PAGE_BYTES;

/// The payloads, one a slot.
pub const RING_SLOT_STRIDE: u32 = RING_CAPACITY * RING_SLOT_BYTES;

/// Three rings: submission, completion, and the spill the completion ring
/// overflows into. Equal, because which way the traffic runs is a question
/// about the workload and the workload is still moving — the renderer came in
/// and what the shell still sends is on its way in behind it.
pub const RING_COUNT: u32 = 3;

const _: () = assert!(ring_bytes(RING_CAPACITY) <= RING_DESC_STRIDE, "a ring's descriptors outgrow their stride");
const _: () = assert!(slots_bytes(RING_CAPACITY, RING_SLOT_BYTES) == RING_SLOT_STRIDE, "a ring's payloads are not its stride");

// Every boundary here is a chosen constant rather than a rounding, so what a
// `page_up` would have guaranteed has to be asserted instead.
const _: () = assert!(
    RING_DESC_AT.is_multiple_of(PAGE_BYTES) && RING_SLOT_AT.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES),
    "a range begins on a page, so every slot in it begins on one"
);
const _: () = assert!(STACK_BYTES <= RING_DESC_AT, "the stack runs into the descriptors");
const _: () = assert!(RING_DESC_AT + RING_COUNT * RING_DESC_STRIDE <= RING_SLOT_AT, "the descriptors run into the payloads");
const _: () = assert!(RING_SLOT_AT + RING_COUNT * RING_SLOT_STRIDE <= BOOT_AT, "the payloads run into the boot block");
const _: () = assert!(
    BOOT_AT.is_multiple_of(PAGE_BYTES) && BOOT_AT + PAGE_BYTES <= GLOBAL_BASE,
    "the boot block is a page under the bracket's data"
);
