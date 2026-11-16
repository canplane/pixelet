// xpute-link/resource.rs

//! A handle names an object on the side that implements it, which issued it
//! (link.rs). As the component model's table does, a handle's slot is used
//! again once its object is dropped, so that a side issues handles for as
//! long as it runs, the slots dropped first used first, so that each comes
//! back as late as it can; the slot is the handle's low `SLOT_BITS`, from 1,
//! the root being 0. Above it a generation, which a slot's drop advances, keeps a
//! frame late for a dropped object from reaching the one in its slot now,
//! until the slot has been used `GENERATION_MASK` + 1 times. More live objects than
//! the slots hold is a bug.

use crate::frame::spec::{NAMED, SLOT_BITS};

pub type Handle = u32;

const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;
/// The largest generation an issued handle holds above its slot, below `NAMED`.
const GENERATION_MASK: u32 = (NAMED - 1) >> SLOT_BITS;

/// What an issued slot's `next` holds, which no free slot's does.
const ISSUED: u32 = u32::MAX;

/// A slot: its generation, its object while it holds one, and the next free
/// slot while it is free, 0 for none; `ISSUED` while it is issued.
struct Slot<T> {
    generation: u32,
    val: Option<T>,
    next: u32,
}

/// The objects a side implements, by handle: slot `k` at `k - 1`, and the queue
/// of the slots dropped, through their `next`, from `head` to `tail`.
pub struct Table<T> {
    slots: Vec<Slot<T>>,
    head: u32,
    tail: u32,
}

impl<T> Default for Table<T> {
    fn default() -> Table<T> {
        Table::new()
    }
}

impl<T> Table<T> {
    pub const fn new() -> Table<T> {
        Table {
            slots: Vec::new(),
            head: 0,
            tail: 0,
        }
    }

    /// Issues a handle, for `put` to fill; `remove` gives it back unfilled.
    pub fn issue(&mut self) -> Handle {
        let k = if self.head != 0 {
            let k = self.head;
            let s = &mut self.slots[k as usize - 1];
            self.head = core::mem::replace(&mut s.next, ISSUED);
            if self.head == 0 {
                self.tail = 0;
            }
            k
        } else {
            self.slots.push(Slot {
                generation: 0,
                val: None,
                next: ISSUED,
            });
            let k = self.slots.len() as u32;
            xpute_kit::ensure!(k <= SLOT_MASK, EMFILE, k);
            k
        };
        k | self.slots[k as usize - 1].generation << SLOT_BITS
    }

    /// The slot `h` names, where `h` is of its generation.
    fn slot(&mut self, h: Handle) -> Option<&mut Slot<T>> {
        let k = (h & SLOT_MASK) as usize;
        let s = self.slots.get_mut(k.checked_sub(1)?)?;
        (s.generation == h >> SLOT_BITS).then_some(s)
    }

    /// Puts `v` under `h`: a handle this table issued, or one the other end
    /// issued, as a UI's end holds what the runtime opens on it under the
    /// guest's end's handles. The slot `h` names is then issued at `h`'s
    /// generation, taken out of the queue if it was free, it and the slots
    /// before it made free where there were none; one issued at another
    /// generation is a bug.
    pub fn put(&mut self, h: Handle, v: T) {
        let k = h & SLOT_MASK;
        xpute_kit::ensure!(k != 0, EBADF, h);
        while (self.slots.len() as u32) < k {
            self.slots.push(Slot {
                generation: 0,
                val: None,
                next: ISSUED,
            });
            self.remove(self.slots.len() as u32);
        }
        let next = self.slots[k as usize - 1].next;
        if next != ISSUED {
            let mut prev = 0;
            let mut c = self.head;
            while c != k {
                prev = c;
                c = self.slots[c as usize - 1].next;
            }
            match prev {
                0 => self.head = next,
                p => self.slots[p as usize - 1].next = next,
            }
            if self.tail == k {
                self.tail = prev;
            }
            let s = &mut self.slots[k as usize - 1];
            s.next = ISSUED;
            s.generation = h >> SLOT_BITS;
        }
        let s = &mut self.slots[k as usize - 1];
        xpute_kit::ensure!(s.generation == h >> SLOT_BITS, EBADF, h);
        s.val = Some(v);
    }

    /// Puts `v` at a handle issued for it, and answers the handle.
    pub fn add(&mut self, v: T) -> Handle {
        let h = self.issue();
        self.put(h, v);
        h
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        let k = (h & SLOT_MASK) as usize;
        let s = self.slots.get(k.checked_sub(1)?)?;
        s.val.as_ref().filter(|_| s.generation == h >> SLOT_BITS)
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        self.slot(h)?.val.as_mut()
    }

    /// Drops `h`: its object, if it holds one, and its slot, last in the
    /// queue, for a later handle of the next generation. A slot that is free
    /// already is no handle's.
    pub fn remove(&mut self, h: Handle) -> Option<T> {
        let s = self.slot(h).filter(|s| s.next == ISSUED)?;
        let v = s.val.take();
        s.generation = (s.generation + 1) & GENERATION_MASK;
        s.next = 0;
        let k = h & SLOT_MASK;
        match self.tail {
            0 => self.head = k,
            t => self.slots[t as usize - 1].next = k,
        }
        self.tail = k;
        v
    }
}
