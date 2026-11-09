// xpute-core/abi/handle.rs

//! A handle is a slot and that slot's generation, so a handle kept past its
//! release names nothing that takes the slot later. The other side reads only
//! the slot. 0 is never a handle.

use core::mem::MaybeUninit;

/// The low bits of a handle, which hold its slot. The high 16 bits hold the
/// generation, which is therefore a `u16`. The runtime splits a handle the
/// same way.
pub const SLOT_BITS: u32 = 16;
const _: () = assert!(
    SLOT_BITS + u16::BITS == u32::BITS,
    "a slot and a generation fill a handle"
);
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;

pub fn handle_slot(handle: u32) -> u32 {
    handle & SLOT_MASK
}

fn handle_of(slot: u32, generation: u16) -> u32 {
    slot | ((generation as u32) << SLOT_BITS)
}

struct Slot<T> {
    generation: u16,
    /// One more than the epoch in which the slot was taken, or 0 while the
    /// slot is free. It is a `u64` so that no program lives through enough
    /// epochs for the count to wrap round to a stale value.
    taken_epoch: u64,
    /// The next slot in the free list. Slot 0 is never handed out, so 0 ends
    /// the list.
    next: u32,
    /// Initialized while `taken_epoch` is not 0.
    val: MaybeUninit<T>,
}

/// A table has N slots, each holding a `T` while its handle is live. A slot
/// given back is the next one taken. All-zero bytes are an empty table, so a
/// table can lie in memory that starts zeroed, or be emptied by zeroing it,
/// without ever being built and moved.
pub struct Table<T: Copy, const N: usize> {
    slot: [Slot<T>; N],
    /// The current transaction, advanced by `begin`. The slots taken since
    /// then are the ones the transaction made.
    epoch: u64,
    free: u32,
    /// The highest slot ever handed out; the slots past it never have been.
    top: u32,
    cnt: u32,
}

impl<T: Copy, const N: usize> Default for Table<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy, const N: usize> Table<T, N> {
    pub const fn new() -> Self {
        const {
            assert!(
                N >= 2 && N <= 1 << SLOT_BITS,
                "a slot is 16 bits, and slot 0 is never handed out"
            )
        };
        Table {
            slot: [const {
                Slot {
                    generation: 0,
                    taken_epoch: 0,
                    next: 0,
                    val: MaybeUninit::uninit(),
                }
            }; N],
            epoch: 0,
            free: 0,
            top: 0,
            cnt: 0,
        }
    }

    /// `insert` answers none when every slot is taken.
    pub fn insert(&mut self, val: T) -> Option<u32> {
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
        // The generation is never 0, so no handle is 0.
        s.generation = s.generation.wrapping_add(1).max(1);
        s.taken_epoch = self.epoch + 1;
        s.val = MaybeUninit::new(val);
        self.cnt += 1;
        Some(handle_of(slot, s.generation))
    }

    pub fn live(&self, handle: u32) -> bool {
        let s = handle_slot(handle) as usize;
        s != 0
            && s < N
            && self.slot[s].taken_epoch != 0
            && self.slot[s].generation as u32 == handle >> SLOT_BITS
    }

    pub fn get(&self, handle: u32) -> Option<&T> {
        // SAFETY: a live slot's value is initialized.
        self.live(handle).then(|| unsafe {
            self.slot[handle_slot(handle) as usize]
                .val
                .assume_init_ref()
        })
    }

    pub fn get_mut(&mut self, handle: u32) -> Option<&mut T> {
        // SAFETY: as `get`.
        self.live(handle).then(|| unsafe {
            self.slot[handle_slot(handle) as usize]
                .val
                .assume_init_mut()
        })
    }

    pub fn remove(&mut self, handle: u32) -> Option<T> {
        let val = *self.get(handle)?;
        let slot = handle_slot(handle);
        let s = &mut self.slot[slot as usize];
        s.taken_epoch = 0;
        s.next = self.free;
        self.free = slot;
        self.cnt -= 1;
        Some(val)
    }

    /// Begins a transaction: the slots taken from here on are the ones
    /// `rollback` gives back.
    pub fn begin(&mut self) {
        self.epoch += 1;
    }

    /// Removes every handle taken since `begin` and not yet removed, and
    /// hands each to `release`. These are what a transaction that did not commit
    /// made, which the other side never saw.
    pub fn rollback(&mut self, mut release: impl FnMut(u32, T)) {
        for slot in 1..=self.top {
            let s = &self.slot[slot as usize];
            let handle = handle_of(slot, s.generation);
            if s.taken_epoch == self.epoch + 1 {
                if let Some(val) = self.remove(handle) {
                    release(handle, val);
                }
            }
        }
    }

    pub fn cnt(&self) -> u32 {
        self.cnt
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
