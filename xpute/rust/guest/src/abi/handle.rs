// xpute-guest/abi/handle.rs

//! Generational handles: a slot and its generation, so a handle kept past its
//! release names nothing that takes the slot after. 0 is never a handle.
//! `HandleTable` is fixed at N slots for what another side keeps; `Slots`
//! grows and holds values this side keeps.

/// The rest is the generation, hence its `u16` lane. The host uses the same 16.
pub const SLOT_BITS: u32 = 16;
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;

pub fn handle_slot(handle: u32) -> u32 {
    handle & SLOT_MASK
}

fn handle_of(slot: u32, generation: u16) -> u32 {
    slot | ((generation as u32) << SLOT_BITS)
}

/// Never 0, so no handle is 0.
fn next_generation(generation: u16) -> u16 {
    generation.wrapping_add(1).max(1)
}

/// All-zero bytes are an empty table, so one may live in memory that starts
/// zeroed, or be emptied by zeroing it, and is never built and moved.
pub struct HandleTable<const N: usize> {
    generation: [u16; N],
    /// One past the epoch a live slot was taken in; 0 for a free one. A `u64`
    /// so no count of epochs a program lives through brings a stale one round.
    made: [u64; N],
    /// A free slot's next free slot; 0, which is never handed out, ends the list.
    next: [u32; N],
    /// Advanced by `begin`: the slots taken since are those made in it.
    epoch: u64,
    free: u32,
    /// The highest slot handed out; those past it never have been.
    top: u32,
    count: u32,
}

impl<const N: usize> Default for HandleTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> HandleTable<N> {
    pub const fn new() -> Self {
        const { assert!(N >= 2 && N <= 1 << SLOT_BITS, "a slot is 16 bits, and slot 0 is never handed out") };
        HandleTable {
            generation: [0; N],
            made: [0; N],
            next: [0; N],
            epoch: 0,
            free: 0,
            top: 0,
            count: 0,
        }
    }

    pub fn acquire(&mut self) -> Option<u32> {
        let slot = if self.free != 0 {
            let slot = self.free;
            self.free = self.next[slot as usize];
            slot
        } else if (self.top as usize) + 1 < N {
            self.top += 1;
            self.top
        } else {
            return None;
        };
        let s = slot as usize;
        self.generation[s] = next_generation(self.generation[s]);
        self.made[s] = self.epoch + 1;
        self.count += 1;
        Some(handle_of(slot, self.generation[s]))
    }

    pub fn release(&mut self, handle: u32) -> bool {
        if !self.live(handle) {
            return false;
        }
        let slot = handle_slot(handle);
        self.made[slot as usize] = 0;
        self.next[slot as usize] = self.free;
        self.free = slot;
        self.count -= 1;
        true
    }

    pub fn live(&self, handle: u32) -> bool {
        let s = handle_slot(handle) as usize;
        s != 0 && s < N && self.made[s] != 0 && self.generation[s] as u32 == handle >> SLOT_BITS
    }

    /// From here, what is taken is what `rollback` gives back.
    pub fn begin(&mut self) {
        self.epoch += 1;
    }

    /// Every handle taken since `begin` and not yet released, released: what a
    /// transaction that did not commit made, which the other side never saw.
    pub fn rollback(&mut self) {
        for s in 1..=self.top {
            if self.made[s as usize] == self.epoch + 1 {
                self.release(handle_of(s, self.generation[s as usize]));
            }
        }
    }

    pub fn count(&self) -> u32 {
        self.count
    }
}

/// Grows, never shrinks; a slot given back is the next one taken.
pub struct Slots<T> {
    /// Slot 0 is held empty so no handle is 0.
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

    pub fn insert(&mut self, value: T) -> u32 {
        if self.entries.is_empty() {
            self.entries.push((0, None));
        }
        let slot = self.free.pop().unwrap_or_else(|| {
            xpute_kit::ensure!(self.entries.len() < 1 << SLOT_BITS, ENOSPC, SLOT_BITS);
            self.entries.push((0, None));
            (self.entries.len() - 1) as u32
        });
        let entry = &mut self.entries[slot as usize];
        entry.0 = next_generation(entry.0);
        entry.1 = Some(value);
        handle_of(slot, entry.0)
    }

    fn entry(&self, handle: u32) -> Option<&(u16, Option<T>)> {
        self.entries
            .get(handle_slot(handle) as usize)
            .filter(|(generation, value)| value.is_some() && *generation as u32 == handle >> SLOT_BITS)
    }

    pub fn get(&self, handle: u32) -> Option<&T> {
        self.entry(handle)?.1.as_ref()
    }

    pub fn get_mut(&mut self, handle: u32) -> Option<&mut T> {
        self.entry(handle)?;
        self.entries[handle_slot(handle) as usize].1.as_mut()
    }

    pub fn remove(&mut self, handle: u32) -> Option<T> {
        self.entry(handle)?;
        let slot = handle_slot(handle);
        self.free.push(slot);
        self.entries[slot as usize].1.take()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(slot, (generation, value))| Some((handle_of(slot as u32, *generation), value.as_ref()?)))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entries.iter_mut().filter_map(|(_, value)| value.as_mut())
    }
}

#[cfg(test)]
#[path = "handle.test.rs"]
mod test;
