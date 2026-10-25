// xpute-guest/abi/handle.rs

//! Generational handles: a slot and its generation, so a handle kept past its
//! release names nothing that takes the slot after. 0 is never a handle.

use core::mem::MaybeUninit;

/// The rest is the generation, hence its `u16`. The host uses the same 16.
pub const SLOT_BITS: u32 = 16;
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;

pub fn handle_slot(handle: u32) -> u32 {
    handle & SLOT_MASK
}

fn handle_of(slot: u32, generation: u16) -> u32 {
    slot | ((generation as u32) << SLOT_BITS)
}

struct Slot<T> {
    generation: u16,
    /// One past the epoch the slot was taken in; 0 while free. A `u64`, so
    /// no count of epochs a program lives through brings a stale one round.
    made: u64,
    /// A free slot's next free slot; 0, never handed out, ends the list.
    next: u32,
    /// Initialized while `made` is not 0.
    value: MaybeUninit<T>,
}

/// N slots, each holding a `T` while its handle is live; a slot given back is
/// the next one taken. All-zero bytes are an empty table, so one may lie in
/// memory that starts zeroed, or be emptied by zeroing it, never built and
/// moved.
pub struct Table<T: Copy, const N: usize> {
    slot: [Slot<T>; N],
    /// Advanced by `begin`: the slots taken since are those made in it.
    epoch: u64,
    free: u32,
    /// The highest slot handed out; those past it never have been.
    top: u32,
    count: u32,
}

impl<T: Copy, const N: usize> Default for Table<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy, const N: usize> Table<T, N> {
    pub const fn new() -> Self {
        const { assert!(N >= 2 && N <= 1 << SLOT_BITS, "a slot is 16 bits, and slot 0 is never handed out") };
        Table {
            slot: [const {
                Slot {
                    generation: 0,
                    made: 0,
                    next: 0,
                    value: MaybeUninit::uninit(),
                }
            }; N],
            epoch: 0,
            free: 0,
            top: 0,
            count: 0,
        }
    }

    /// None when every slot is taken.
    pub fn insert(&mut self, value: T) -> Option<u32> {
        let slot = if self.free != 0 {
            let slot = self.free;
            self.free = self.slot[slot as usize].next;
            slot
        } else if (self.top as usize) + 1 < N {
            self.top += 1;
            self.top
        } else {
            return None;
        };
        let s = &mut self.slot[slot as usize];
        // Never 0, so no handle is 0.
        s.generation = s.generation.wrapping_add(1).max(1);
        s.made = self.epoch + 1;
        s.value = MaybeUninit::new(value);
        self.count += 1;
        Some(handle_of(slot, s.generation))
    }

    pub fn live(&self, handle: u32) -> bool {
        let s = handle_slot(handle) as usize;
        s != 0 && s < N && self.slot[s].made != 0 && self.slot[s].generation as u32 == handle >> SLOT_BITS
    }

    pub fn get(&self, handle: u32) -> Option<&T> {
        // SAFETY: a live slot's value is initialized.
        self.live(handle).then(|| unsafe { self.slot[handle_slot(handle) as usize].value.assume_init_ref() })
    }

    pub fn get_mut(&mut self, handle: u32) -> Option<&mut T> {
        // SAFETY: as `get`.
        self.live(handle).then(|| unsafe { self.slot[handle_slot(handle) as usize].value.assume_init_mut() })
    }

    pub fn remove(&mut self, handle: u32) -> Option<T> {
        let value = *self.get(handle)?;
        let slot = handle_slot(handle);
        let s = &mut self.slot[slot as usize];
        s.made = 0;
        s.next = self.free;
        self.free = slot;
        self.count -= 1;
        Some(value)
    }

    /// From here, what is taken is what `rollback` gives back.
    pub fn begin(&mut self) {
        self.epoch += 1;
    }

    /// Every handle taken since `begin` and not yet removed, removed and
    /// handed to `gone`: what a transaction that did not commit made, which
    /// the other side never saw.
    pub fn rollback(&mut self, mut gone: impl FnMut(u32, T)) {
        for slot in 1..=self.top {
            let s = &self.slot[slot as usize];
            let handle = handle_of(slot, s.generation);
            if s.made == self.epoch + 1 {
                if let Some(value) = self.remove(handle) {
                    gone(handle, value);
                }
            }
        }
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        (1..=self.top).filter_map(|slot| {
            let handle = handle_of(slot, self.slot[slot as usize].generation);
            Some((handle, self.get(handle)?))
        })
    }
}

#[cfg(test)]
#[path = "handle.test.rs"]
mod test;
