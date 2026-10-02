// pxl-kernel/door/layout.rs

//! The front of memory, below GLOBAL_BASE: the rings' descriptors, their
//! slot payloads and the directory's page, at fixed addresses from mem.json.

use xpute_runtime::ipc::doorbell::{DoorLayout, RingLayout};
use xpute_runtime::ipc::ring::{ring_bytes, slots_bytes};

use crate::mem::spec::{COMPLETION_CAPACITY, DIRECTORY_AT, GLOBAL_BASE, PAGE_BYTES, RING_DESC_AT, RING_SLOT_AT, STACK_BYTES, SUBMISSION_CAPACITY};

/// A packet is at most a page; anything larger crosses by address. A page
/// is also the buddy's smallest block.
pub const RING_SLOT_BYTES: u32 = PAGE_BYTES;

const fn page_up(bytes: u32) -> u32 {
    bytes.div_ceil(PAGE_BYTES) * PAGE_BYTES
}

const SUBMISSION_DESC_AT: u32 = RING_DESC_AT;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(SUBMISSION_CAPACITY));
const DESC_END: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(COMPLETION_CAPACITY));

const SUBMISSION_SLOT_AT: u32 = RING_SLOT_AT;
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(SUBMISSION_CAPACITY, RING_SLOT_BYTES);
const SLOT_END: u32 = COMPLETION_SLOT_AT + slots_bytes(COMPLETION_CAPACITY, RING_SLOT_BYTES);

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

// The boundaries are chosen constants, not roundings, so page alignment is asserted.
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
