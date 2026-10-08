// xpute-guest/abi/handle.rs

//! Generational handles: a slot and its generation, so a handle kept past its
//! release names nothing that takes the slot after. 0 is never a handle.
//! `HandleTable` is fixed at N slots for what another side keeps; `Slots`
//! grows and holds values this side keeps.

/// The rest is the generation, hence its `u16` lane. The host uses the same 16.
pub const SLOT_BITS: u32 = 16;
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;
const NONE: u32 = u32::MAX;

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

pub struct HandleTable<const N: usize> {
    generation: [u16; N],
    live: [bool; N],
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
        xpute_kit::ensure!((2..=1 << SLOT_BITS).contains(&N), EINVAL, SLOT_BITS);
        HandleTable {
            generation: [0; N],
            live: [false; N],
            next: [NONE; N],
            free: NONE,
            fresh: 1,
            count: 0,
        }
    }

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
        self.generation[s] = next_generation(self.generation[s]);
        self.live[s] = true;
        self.count += 1;
        Some(handle_of(slot, self.generation[s]))
    }

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

    pub fn live(&self, handle: u32) -> bool {
        let s = handle_slot(handle) as usize;
        s != 0 && s < N && self.live[s] && self.generation[s] as u32 == handle >> SLOT_BITS
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
