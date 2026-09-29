// pxl-kernel/door/layout.rs

//! The front of the memory, below where a bracket's own data begins
//! (GLOBAL_BASE): the two rings' descriptors and their slots' payloads, at
//! addresses the layout fixes rather than anyone hands out, and the page the
//! directory is written in. The numbers are mem.json's, the same file the
//! map and the shell read (pxl-tool/bin/spec.rs writes mem/mem.spec.rs).

use xpute_runtime::ipc::doorbell::{DoorLayout, RingLayout};
use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes};

use crate::mem::spec::{COMPLETION_CAPACITY, DIRECTORY_AT, GLOBAL_BASE, PAGE_BYTES, RING_DESC_AT, RING_SLOT_AT, STACK_BYTES, SUBMISSION_CAPACITY};

/// A ring slot's payload is a page: a packet is at most one, and anything
/// larger is bulk and crosses by address. The page is also the buddy's
/// smallest block, so a payload is one block wherever one has to come from.
pub const RING_SLOT_BYTES: u32 = PAGE_BYTES;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

/// The submission ring's descriptors, then the completion ring's, each
/// beginning on a page.
const SUBMISSION_DESC_AT: u32 = RING_DESC_AT;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(SUBMISSION_CAPACITY));
const DESC_END: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(COMPLETION_CAPACITY));

/// The payloads, one a slot, in the same order.
const SUBMISSION_SLOT_AT: u32 = RING_SLOT_AT;
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(SUBMISSION_CAPACITY, RING_SLOT_BYTES);
const SLOT_END: u32 = COMPLETION_SLOT_AT + slots_bytes(COMPLETION_CAPACITY, RING_SLOT_BYTES);

/// Where the door's rings lie.
pub const DOOR_LAYOUT: DoorLayout = DoorLayout {
    slot_bytes: RING_SLOT_BYTES,
    submission: RingLayout {
        capacity: SUBMISSION_CAPACITY,
        desc_at: SUBMISSION_DESC_AT,
        slot_at: SUBMISSION_SLOT_AT,
    },
    completion: RingLayout {
        capacity: COMPLETION_CAPACITY,
        desc_at: COMPLETION_DESC_AT,
        slot_at: COMPLETION_SLOT_AT,
    },
};

// Every boundary here is a chosen constant rather than a rounding, so what a
// `page_up` would have guaranteed has to be asserted instead.
const _: () = assert!(
    RING_DESC_AT.is_multiple_of(PAGE_BYTES) && RING_SLOT_AT.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES),
    "a range begins on a page, so every slot in it begins on one"
);
const _: () = assert!(STACK_BYTES <= RING_DESC_AT, "the stack runs into the descriptors");
const _: () = assert!(DESC_END <= RING_SLOT_AT, "the descriptors run into the payloads");
const _: () = assert!(SLOT_END <= DIRECTORY_AT, "the payloads run into the directory");
const _: () = assert!(
    DIRECTORY_AT.is_multiple_of(PAGE_BYTES) && DIRECTORY_AT + PAGE_BYTES <= GLOBAL_BASE,
    "the directory is a page under the bracket's data"
);
