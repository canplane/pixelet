// xpute-runtime/abi/handle.rs

//! Handles to what another side keeps: the number is the table's, the thing it
//! names is the holder's — the way an OS hands out a descriptor and keeps the
//! file. A handle packs a slot and that slot's generation, so a handle kept
//! past its release does not name whatever took the slot after it.
//!
//! The table is fixed at N slots and never grows: past them `acquire` refuses.
//! Slot 0 is never handed out, so 0 is never a handle.
//!
//! `Slots` is the same handle over values this side keeps: it grows, and a
//! removed value's handle names nothing whatever takes its slot next.

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
        xpute_core::ensure!((2..=1 << SLOT_BITS).contains(&N), EINVAL, SLOT_BITS);
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

/// Values held by handle: what a table of `Vec<Option<T>>` indexed by position
/// would be, except that a handle kept past its value's removal names nothing
/// rather than whatever was put in its slot after. The handle is the table's
/// above: a slot and that slot's generation, 0 never one. It grows as values
/// are put in and never shrinks; a slot given back is the next one taken.
pub struct Slots<T> {
    /// By slot: its generation, and its value while one is in it. Slot 0 is
    /// held empty so no handle is 0.
    entries: Vec<(u16, Option<T>)>,
    free: Vec<u32>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Slots<T> {
    pub const fn new() -> Self {
        Slots {
            entries: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Puts `value` in a slot of its own: its handle.
    pub fn insert(&mut self, value: T) -> u32 {
        if self.entries.is_empty() {
            self.entries.push((0, None));
        }
        let slot = self.free.pop().unwrap_or_else(|| {
            xpute_core::ensure!(self.entries.len() < 1 << SLOT_BITS, ENOSPC, SLOT_BITS);
            self.entries.push((0, None));
            (self.entries.len() - 1) as u32
        });
        let entry = &mut self.entries[slot as usize];
        entry.0 = entry.0.wrapping_add(1).max(1);
        entry.1 = Some(value);
        slot | ((entry.0 as u32) << SLOT_BITS)
    }

    fn entry(&self, handle: u32) -> Option<&(u16, Option<T>)> {
        self.entries
            .get(handle_slot(handle) as usize)
            .filter(|(generation, value)| value.is_some() && *generation as u32 == handle >> SLOT_BITS)
    }

    /// The value the handle names, or none once it was removed.
    pub fn get(&self, handle: u32) -> Option<&T> {
        self.entry(handle)?.1.as_ref()
    }

    pub fn get_mut(&mut self, handle: u32) -> Option<&mut T> {
        self.entry(handle)?;
        self.entries[handle_slot(handle) as usize].1.as_mut()
    }

    /// Takes the value out and gives its slot back; none for a handle that
    /// names nothing.
    pub fn remove(&mut self, handle: u32) -> Option<T> {
        self.entry(handle)?;
        let slot = handle_slot(handle);
        self.free.push(slot);
        self.entries[slot as usize].1.take()
    }

    /// Every value held, with its handle.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(slot, (generation, value))| Some((slot as u32 | ((*generation as u32) << SLOT_BITS), value.as_ref()?)))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entries.iter_mut().filter_map(|(_, value)| value.as_mut())
    }
}

#[cfg(test)]
#[path = "handle.test.rs"]
mod test;
