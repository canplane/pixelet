// xpute-guest/ipc/ring.rs

//! A ring of frames, laid out by spec/ipc/ring.json. A push fails only
//! when full (EAGAIN) or when the packet exceeds a slot (EMSGSIZE), never
//! depending on what was sent before.

#[path = "ring.spec.rs"]
mod spec;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use super::frame::{self, cmd_of, cmd_word, flags_of, Frame};
use crate::mem::Memory;
use spec::{CAP, HEAD, HEADER_WORDS, SLOT_BASE, SLOT_BYTES, TAIL};

pub const fn ring_bytes(cap: u32) -> u32 {
    (HEADER_WORDS + cap * frame::WORDS) * 4
}

pub const fn slots_bytes(cap: u32, slot_bytes: u32) -> u32 {
    cap * slot_bytes
}

/// Payloads are padded to 8 bytes.
fn padded(bytes: usize) -> usize {
    bytes.next_multiple_of(8)
}

/// In u64: a length word near 2^32 must read as too long, not wrap on wasm32.
fn packet_bytes(u8: &[u8], at: usize) -> u64 {
    let payload = u32::from_le_bytes([u8[at + 8], u8[at + 9], u8[at + 10], u8[at + 11]]);
    (16 + u64::from(payload)).next_multiple_of(8)
}

#[derive(Clone)]
pub struct Ring {
    mem: Memory,
    at: usize,
    pub cap: u32,
    pub slot_bytes: u32,
    mask: u32,
    slot_base: u32,
}

impl Ring {
    /// # Safety
    /// The ring's words at `at` and its slots in `mem` must stay valid, and
    /// never move, for as long as the ring is used.
    pub unsafe fn new(mem: Memory, at: usize) -> Ring {
        let header = |word: u32| mem.word(at + 4 * word as usize);
        let cap = header(CAP);
        if !cap.is_power_of_two() {
            xpute_kit::bug!(EINVAL, cap, at);
        }
        Ring {
            mem,
            at,
            cap,
            slot_bytes: header(SLOT_BYTES),
            mask: cap - 1,
            slot_base: header(SLOT_BASE),
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn words(&self) -> &mut [u32] {
        // SAFETY: the memory outlives the ring, and each side writes only its
        // own words.
        unsafe { core::slice::from_raw_parts_mut(self.mem.at::<u32>(self.at), ring_bytes(self.cap) as usize / 4) }
    }

    #[allow(clippy::mut_from_ref)]
    fn slots(&self) -> &mut [u8] {
        // SAFETY: as `words`.
        unsafe { core::slice::from_raw_parts_mut(self.mem.at::<u8>(self.slot_base as usize), slots_bytes(self.cap, self.slot_bytes) as usize) }
    }

    /// # Safety
    /// As `new`.
    pub unsafe fn init(mem: Memory, at: usize, cap: u32, slot_base: u32, slot_bytes: u32) -> Ring {
        if !cap.is_power_of_two() {
            xpute_kit::bug!(EINVAL, cap);
        }
        if !slot_bytes.is_power_of_two() || !slot_bytes.is_multiple_of(8) {
            xpute_kit::bug!(EINVAL, slot_bytes);
        }
        // SAFETY: the caller's memory, as `new` requires.
        unsafe {
            core::slice::from_raw_parts_mut(mem.at::<u8>(at), ring_bytes(cap) as usize).fill(0);
            let set_header = |word: u32, w: u32| mem.set_word(at + 4 * word as usize, w);
            set_header(CAP, cap);
            set_header(SLOT_BYTES, slot_bytes);
            set_header(SLOT_BASE, slot_base);
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

    /// The word, from the ring's start, where the frame at `position` begins.
    pub fn entry_at(&self, position: u32) -> u32 {
        HEADER_WORDS + (position & self.mask) * frame::WORDS
    }

    fn slot_at(&self, position: u32) -> usize {
        ((position & self.mask) * self.slot_bytes) as usize
    }

    pub fn push(&self, tag: u32, cmd: u32, flags: u32, result: i32, packet: Option<&[u8]>) -> Errno {
        if self.len() >= self.cap {
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
        if self.len() >= self.cap {
            return Errno::EAGAIN;
        }
        let start = self.slot_at(self.tail());
        let Some(n) = write(&mut self.slots()[start..start + self.slot_bytes as usize]).filter(|&n| n <= self.slot_bytes) else {
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

    /// The head's frame, none when empty. Read it, then `advance`.
    pub fn peek(&self) -> Option<Frame> {
        if self.is_empty() {
            return None;
        }
        let w = self.words();
        let at = self.entry_at(self.head()) as usize;
        let word = w[at + frame::CMD as usize];
        Some(Frame {
            tag: w[at + frame::TAG as usize],
            cmd: cmd_of(word),
            flags: flags_of(word),
            result: w[at + frame::RESULT as usize] as i32,
            packet_at: w[at + frame::PACKET as usize],
        })
    }

    /// Valid until `advance`; `'static` because the memory's lifetime is
    /// `new`'s contract, and a reader holds the packet while it acts on the ring.
    /// EBADMSG when the frame names bytes outside a slot.
    pub fn packet(&self, f: &Frame) -> Result<Option<&'static [u8]>, MarshalError> {
        if f.packet_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG);
        let off = f.packet_at.checked_sub(self.slot_base).ok_or_else(bad)? as usize;
        if !off.is_multiple_of(self.slot_bytes as usize) || off + 16 > self.slots().len() {
            return Err(bad());
        }
        let bytes = packet_bytes(self.slots(), off);
        if bytes > u64::from(self.slot_bytes) {
            return Err(bad());
        }
        let bytes = bytes as usize;
        // SAFETY: the memory `new` was given, as its contract requires.
        Ok(Some(unsafe { core::slice::from_raw_parts(self.mem.at::<u8>(self.slot_base as usize + off), bytes) }))
    }

    pub fn advance(&self) {
        if self.is_empty() {
            xpute_kit::bug!(ENOTRECOVERABLE);
        }
        self.words()[HEAD as usize] = self.head().wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
