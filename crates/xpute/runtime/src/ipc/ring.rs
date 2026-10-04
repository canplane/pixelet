// xpute-runtime/ipc/ring.rs

//! A ring of frames, laid out by spec/xpute/ipc/ring.json. A push fails only
//! when full (EAGAIN) or when the packet exceeds a slot (EMSGSIZE), never
//! depending on what was sent before.

#[path = "ring.spec.rs"]
mod spec;

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

use super::frame::{self, cmd_of, cmd_word, flags_of};
use spec::{CAPACITY, HEAD, HEADER_WORDS, SLOT_BASE, SLOT_BYTES, TAIL};

/// Arithmetic on the address, not the pointer: the base may be wasm address
/// 0, where `ptr::add` is UB, and a release build once dropped the capacity
/// word for it, reading back a ring of capacity 0.
fn at_of<T>(mem: *mut u8, off: usize) -> *mut T {
    core::ptr::with_exposed_provenance_mut(mem.expose_provenance() + off)
}

pub const fn ring_bytes(capacity: u32) -> u32 {
    (HEADER_WORDS + capacity * frame::WORDS) * 4
}

pub const fn slots_bytes(capacity: u32, slot_bytes: u32) -> u32 {
    capacity * slot_bytes
}

/// Payloads are padded to 8 bytes.
fn padded(bytes: usize) -> usize {
    bytes.next_multiple_of(8)
}

fn packet_bytes(u8: &[u8], at: usize) -> usize {
    let payload = u32::from_le_bytes([u8[at + 8], u8[at + 9], u8[at + 10], u8[at + 11]]);
    padded(16 + payload as usize)
}

#[derive(Clone)]
pub struct Ring {
    mem: *mut u8,
    at: u32,
    pub capacity: u32,
    pub slot_bytes: u32,
    mask: u32,
    slot_base: u32,
}

impl Ring {
    /// # Safety
    /// `mem` must stay valid, and never move, for as long as the ring is used.
    pub unsafe fn new(mem: *mut u8, at: u32) -> Ring {
        let head = unsafe { core::slice::from_raw_parts(at_of::<u32>(mem, at as usize), HEADER_WORDS as usize) };
        let capacity = head[CAPACITY as usize];
        if !capacity.is_power_of_two() {
            xpute_core::bug!(EINVAL, capacity, at);
        }
        Ring {
            mem,
            at,
            capacity,
            slot_bytes: head[SLOT_BYTES as usize],
            mask: capacity - 1,
            slot_base: head[SLOT_BASE as usize],
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn words(&self) -> &mut [u32] {
        // SAFETY: the memory outlives the ring, and each side writes only its
        // own words.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u32>(self.mem, self.at as usize), ring_bytes(self.capacity) as usize / 4) }
    }

    #[allow(clippy::mut_from_ref)]
    fn slots(&self) -> &mut [u8] {
        // SAFETY: as `words`.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u8>(self.mem, self.slot_base as usize), slots_bytes(self.capacity, self.slot_bytes) as usize) }
    }

    /// # Safety
    /// As `new`.
    pub unsafe fn init(mem: *mut u8, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32) -> Ring {
        if !capacity.is_power_of_two() {
            xpute_core::bug!(EINVAL, capacity);
        }
        if !slot_bytes.is_power_of_two() || !slot_bytes.is_multiple_of(8) {
            xpute_core::bug!(EINVAL, slot_bytes);
        }
        // SAFETY: the caller's memory, as `new` requires.
        unsafe {
            core::slice::from_raw_parts_mut(at_of::<u8>(mem, at as usize), ring_bytes(capacity) as usize).fill(0);
            let head = core::slice::from_raw_parts_mut(at_of::<u32>(mem, at as usize), HEADER_WORDS as usize);
            head[CAPACITY as usize] = capacity;
            head[SLOT_BYTES as usize] = slot_bytes;
            head[SLOT_BASE as usize] = slot_base;
            Ring::new(mem, at)
        }
    }

    pub fn head(&self) -> u32 {
        self.words()[HEAD as usize]
    }

    pub fn tail(&self) -> u32 {
        self.words()[TAIL as usize]
    }

    pub fn len(&self) -> u32 {
        self.tail().wrapping_sub(self.head())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entry_at(&self, position: u32) -> u32 {
        HEADER_WORDS + (position & self.mask) * frame::WORDS
    }

    fn slot_at(&self, position: u32) -> usize {
        ((position & self.mask) * self.slot_bytes) as usize
    }

    pub fn push(&self, tag: u32, cmd: u32, flags: u32, packet: Option<&[u8]>, result: i32) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let packet_at = match packet {
            None => 0,
            Some(packet) => {
                if padded(packet.len()) > self.slot_bytes as usize {
                    return Errno::EMSGSIZE;
                }
                let start = self.slot_at(self.tail());
                self.slots()[start..start + packet.len()].copy_from_slice(packet);
                self.pad(start, packet.len());
                self.slot_base + start as u32
            }
        };
        self.publish(tag, cmd, flags, result, packet_at);
        Errno::OK
    }

    /// `write` fills the slot's payload and answers the packet's length, or
    /// none when it did not fit.
    pub fn push_in_place(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: impl FnOnce(&mut [u8]) -> Option<u32>) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let start = self.slot_at(self.tail());
        let Some(n) = write(&mut self.slots()[start..start + self.slot_bytes as usize]) else {
            return Errno::EMSGSIZE;
        };
        self.pad(start, n as usize);
        self.publish(tag, cmd, flags, result, self.slot_base + start as u32);
        Errno::OK
    }

    /// Zeroes the slot from the packet's end to its padded end.
    fn pad(&self, start: usize, len: usize) {
        self.slots()[start + len..start + padded(len)].fill(0);
    }

    /// Writes the tail's frame and moves the tail past it.
    fn publish(&self, tag: u32, cmd: u32, flags: u32, result: i32, packet_at: u32) {
        let w = self.words();
        let at = self.entry_at(w[TAIL as usize]) as usize;
        w[at + frame::TAG as usize] = tag;
        w[at + frame::CMD as usize] = cmd_word(cmd, flags);
        w[at + frame::RESULT as usize] = result as u32;
        w[at + frame::PACKET as usize] = packet_at;
        w[TAIL as usize] = w[TAIL as usize].wrapping_add(1);
    }

    /// -1 when empty.
    pub fn peek(&self) -> i64 {
        if self.is_empty() {
            -1
        } else {
            self.entry_at(self.head()) as i64
        }
    }

    pub fn tag(&self, at: u32) -> u32 {
        self.words()[(at + frame::TAG) as usize]
    }

    pub fn cmd(&self, at: u32) -> u32 {
        cmd_of(self.words()[(at + frame::CMD) as usize])
    }

    pub fn flags(&self, at: u32) -> u32 {
        flags_of(self.words()[(at + frame::CMD) as usize])
    }

    pub fn result(&self, at: u32) -> i32 {
        self.words()[(at + frame::RESULT) as usize] as i32
    }

    /// Valid until `advance`; `'static` because the memory's lifetime is
    /// `new`'s contract, and a reader holds the packet while it acts on the ring.
    pub fn packet(&self, at: u32) -> Result<Option<&'static [u8]>, MarshalError> {
        let packet_at = self.words()[(at + frame::PACKET) as usize];
        if packet_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG);
        let off = packet_at.checked_sub(self.slot_base).ok_or_else(bad)? as usize;
        if !off.is_multiple_of(self.slot_bytes as usize) || off + 16 > self.slots().len() {
            return Err(bad());
        }
        let bytes = packet_bytes(self.slots(), off);
        if bytes > self.slot_bytes as usize {
            return Err(bad());
        }
        // SAFETY: the memory `new` was given, as its contract requires.
        Ok(Some(unsafe { core::slice::from_raw_parts(at_of::<u8>(self.mem, self.slot_base as usize + off), bytes) }))
    }

    pub fn advance(&self) {
        if self.is_empty() {
            xpute_core::bug!(ENOTRECOVERABLE);
        }
        self.words()[HEAD as usize] = self.head().wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
