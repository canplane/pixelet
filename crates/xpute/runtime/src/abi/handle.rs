// xpute-runtime/abi/handle.rs

//! Handles to what another side keeps: the number is the table's, the thing it
//! names is the holder's — the way an OS hands out a descriptor and keeps the
//! file. A handle packs a slot and that slot's generation, so a handle kept
//! past its release does not name whatever took the slot after it.
//!
//! The table is fixed at N slots and never grows: past them `acquire` refuses.
//! Slot 0 is never handed out, so 0 is never a handle.

/// Bits of a handle that are its slot; the rest are the slot's generation.
/// Half of the u32 each way, and the generation lane is `u16` to match: the
/// two are one number written twice, not a parameter. The host reads slots
/// with the same 16 (`@xpute/runtime/abi/handle.ts`).
pub const SLOT_BITS: u32 = 16;
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;
/// No slot: the end of the free list.
const NONE: u32 = u32::MAX;

/// The slot a handle names.
pub fn handle_slot(handle: u32) -> u32 {
    handle & SLOT_MASK
}

pub struct HandleTable<const N: usize> {
    generation: [u16; N],
    live: [bool; N],
    /// The free list's links, by slot.
    next: [u32; N],
    free: u32,
    /// Slots never handed out start here.
    fresh: u32,
    count: u32,
}

impl<const N: usize> Default for HandleTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> HandleTable<N> {
    pub fn new() -> Self {
        assert!((2..=1 << SLOT_BITS).contains(&N), "a handle table holds 2 to 2^{SLOT_BITS} slots");
        HandleTable {
            generation: [0; N],
            live: [false; N],
            next: [NONE; N],
            free: NONE,
            fresh: 1,
            count: 0,
        }
    }

    /// A handle to a slot of its own, or none when every slot is taken.
    pub fn acquire(&mut self) -> Option<u32> {
        let slot = if self.free != NONE {
            let slot = self.free;
            self.free = self.next[slot as usize];
            slot
        } else if (self.fresh as usize) < N {
            self.fresh += 1;
            self.fresh - 1
        } else {
            return None;
        };
        let s = slot as usize;
        self.generation[s] = self.generation[s].wrapping_add(1).max(1);
        self.live[s] = true;
        self.count += 1;
        Some(slot | ((self.generation[s] as u32) << SLOT_BITS))
    }

    /// Gives the handle's slot back. False for a handle that is not live.
    pub fn release(&mut self, handle: u32) -> bool {
        if !self.live(handle) {
            return false;
        }
        let slot = handle_slot(handle);
        self.live[slot as usize] = false;
        self.next[slot as usize] = self.free;
        self.free = slot;
        self.count -= 1;
        true
    }

    /// Whether the handle names its slot now.
    pub fn live(&self, handle: u32) -> bool {
        let s = handle_slot(handle) as usize;
        s != 0 && s < N && self.live[s] && self.generation[s] as u32 == handle >> SLOT_BITS
    }

    /// Handles live now.
    pub fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
#[path = "handle.test.rs"]
mod test;
