// pxl-gpu-link/handle.rs

//! A handle is a slot and that slot's generation, so a handle kept past its
//! release names nothing that takes the slot later; the host reads such a
//! handle's slot alone (`handle_slot`).

pub use crate::spec::SLOT_BITS;

// The generation is a `u16`.
const _: () = assert!(
    SLOT_BITS + u16::BITS == u32::BITS,
    "a slot and a generation fill a handle"
);
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;

pub fn handle_slot(handle: u32) -> u32 {
    handle & SLOT_MASK
}
