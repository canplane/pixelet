// xpute-runtime/ipc/ring.rs

//! A single-producer, single-consumer ring of frames in memory both sides
//! see. Descriptors and payloads lie apart, as in virtio and AF_XDP; message
//! `n` owns descriptor and payload slot `n & (capacity - 1)`, so a payload is
//! free exactly when its descriptor is.
//!
//! Header words: 0 capacity (power of two), 1 head, 2 tail, 3 slot_bytes
//! (power of two, multiple of 8), 4 slot_base, 5..7 reserved. Positions only
//! grow; each side writes only its own word, so no lock is needed.
//!
//! A push fails only when full (EAGAIN) or when the packet exceeds a slot
//! (EMSGSIZE), never depending on what was sent before.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

use super::frame::{cmd_of, cmd_word, flags_of, FRAME_CMD, FRAME_PACKET, FRAME_RESULT, FRAME_TAG, FRAME_WORDS};

/// Arithmetic on the address, not the pointer: the base may be wasm address
/// 0, where `ptr::add` is UB, and a release build once dropped the capacity
/// word for it, reading back a ring of capacity 0.
fn at_of<T>(mem: *mut u8, off: usize) -> *mut T {
    core::ptr::with_exposed_provenance_mut(mem.expose_provenance() + off)
}

pub const RING_HEADER_WORDS: u32 = 8;

pub const fn ring_bytes(capacity: u32) -> u32 {
    (RING_HEADER_WORDS + capacity * FRAME_WORDS) * 4
}

pub const fn slots_bytes(capacity: u32, slot_bytes: u32) -> u32 {
    capacity * slot_bytes
}

fn packet_bytes(u8: &[u8], at: usize) -> u64 {
    let payload = u32::from_le_bytes([u8[at + 8], u8[at + 9], u8[at + 10], u8[at + 11]]);
    (16 + payload as u64).div_ceil(8) * 8
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
        let head = unsafe { core::slice::from_raw_parts(at_of::<u32>(mem, at as usize), RING_HEADER_WORDS as usize) };
        let capacity = head[0];
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            xpute_core::bug!(EINVAL, capacity, at);
        }
        Ring {
            mem,
            at,
            capacity,
            slot_bytes: head[3],
            mask: capacity - 1,
            slot_base: head[4],
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn words(&self) -> &mut [u32] {
        // SAFETY: the memory outlives the ring, and each side writes only its
        // own words.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u32>(self.mem, self.at as usize), (RING_HEADER_WORDS + self.capacity * FRAME_WORDS) as usize) }
    }

    #[allow(clippy::mut_from_ref)]
    fn slots(&self) -> &mut [u8] {
        // SAFETY: as `words`.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u8>(self.mem, self.slot_base as usize), slots_bytes(self.capacity, self.slot_bytes) as usize) }
    }

    /// # Safety
    /// As `new`.
    pub unsafe fn init(mem: *mut u8, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32) -> Ring {
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            xpute_core::bug!(EINVAL, capacity);
        }
        if slot_bytes == 0 || (slot_bytes & (slot_bytes - 1)) != 0 || !slot_bytes.is_multiple_of(8) {
            xpute_core::bug!(EINVAL, slot_bytes);
        }
        // SAFETY: the caller's memory, as `new` requires.
        unsafe {
            core::slice::from_raw_parts_mut(at_of::<u8>(mem, at as usize), ring_bytes(capacity) as usize).fill(0);
            let head = core::slice::from_raw_parts_mut(at_of::<u32>(mem, at as usize), RING_HEADER_WORDS as usize);
            head[0] = capacity;
            head[3] = slot_bytes;
            head[4] = slot_base;
            Ring::new(mem, at)
        }
    }

    pub fn head(&self) -> u32 {
        self.words()[1]
    }

    pub fn tail(&self) -> u32 {
        self.words()[2]
    }

    pub fn len(&self) -> u32 {
        self.words()[2].wrapping_sub(self.words()[1])
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entry_at(&self, position: u32) -> u32 {
        RING_HEADER_WORDS + (position & self.mask) * FRAME_WORDS
    }

    fn slot_at(&self, position: u32) -> usize {
        ((position & self.mask) * self.slot_bytes) as usize
    }

    pub fn push(&self, tag: u32, cmd: u32, flags: u32, packet: Option<&[u8]>, result: i32) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words();
        let mut packet_at = 0;
        if let Some(packet) = packet {
            let need = packet.len().div_ceil(8) * 8;
            if need > self.slot_bytes as usize {
                return Errno::EMSGSIZE;
            }
            let start = self.slot_at(w[2]);
            let bytes = self.slots();
            bytes[start..start + packet.len()].copy_from_slice(packet);
            bytes[start + packet.len()..start + need].fill(0);
            packet_at = self.slot_base + start as u32;
        }
        let at = self.entry_at(w[2]) as usize;
        w[at + FRAME_TAG as usize] = tag;
        w[at + FRAME_CMD as usize] = cmd_word(cmd, flags);
        w[at + FRAME_RESULT as usize] = result as u32;
        w[at + FRAME_PACKET as usize] = packet_at;
        w[2] = w[2].wrapping_add(1);
        Errno::OK
    }

    /// `write` fills the slot's payload and answers the packet's length, or
    /// none when it did not fit.
    pub fn push_in_place(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: impl FnOnce(&mut [u8]) -> Option<u32>) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words();
        let start = self.slot_at(w[2]);
        let end = start + self.slot_bytes as usize;
        let bytes = self.slots();
        let Some(n) = write(&mut bytes[start..end]) else {
            return Errno::EMSGSIZE;
        };
        let need = (n as usize).div_ceil(8) * 8;
        bytes[start + n as usize..start + need].fill(0);
        let at = self.entry_at(w[2]) as usize;
        w[at + FRAME_TAG as usize] = tag;
        w[at + FRAME_CMD as usize] = cmd_word(cmd, flags);
        w[at + FRAME_RESULT as usize] = result as u32;
        w[at + FRAME_PACKET as usize] = self.slot_base + start as u32;
        w[2] = w[2].wrapping_add(1);
        Errno::OK
    }

    /// -1 when empty.
    pub fn peek(&self) -> i64 {
        if self.is_empty() {
            -1
        } else {
            self.entry_at(self.words()[1]) as i64
        }
    }

    pub fn tag(&self, at: u32) -> u32 {
        self.words()[(at + FRAME_TAG) as usize]
    }

    pub fn cmd(&self, at: u32) -> u32 {
        cmd_of(self.words()[(at + FRAME_CMD) as usize])
    }

    pub fn flags(&self, at: u32) -> u32 {
        flags_of(self.words()[(at + FRAME_CMD) as usize])
    }

    pub fn result(&self, at: u32) -> i32 {
        self.words()[(at + FRAME_RESULT) as usize] as i32
    }

    /// Valid until `advance`; `'static` because the memory's lifetime is
    /// `new`'s contract, and a reader holds the packet while it acts on the ring.
    pub fn packet(&self, at: u32) -> Result<Option<&'static [u8]>, MarshalError> {
        let packet_at = self.words()[(at + FRAME_PACKET) as usize];
        if packet_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG);
        let off = packet_at.checked_sub(self.slot_base).ok_or_else(bad)? as usize;
        if !off.is_multiple_of(self.slot_bytes as usize) || off + 16 > self.slots().len() {
            return Err(bad());
        }
        let bytes = packet_bytes(self.slots(), off);
        if bytes > self.slot_bytes as u64 {
            return Err(bad());
        }
        // SAFETY: the memory `new` was given, as its contract requires.
        Ok(Some(unsafe { core::slice::from_raw_parts(at_of::<u8>(self.mem, self.slot_base as usize + off), bytes as usize) }))
    }

    pub fn advance(&self) {
        if self.is_empty() {
            xpute_core::bug!(ENOTRECOVERABLE);
        }
        self.words()[1] = self.words()[1].wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
