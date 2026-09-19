// xpute-runtime/ipc/ring.rs

//! A single-producer, single-consumer ring of messages in a buffer both
//! sides can see — the submission and completion rings between a shell and
//! its kernel, laid out once and shared by address.
//!
//! A message is a frame (frame.rs), and this is where its four words sit: a
//! slot's descriptor, with word 3 the address of that slot's payload in the
//! memory both sides see. Nothing is added to it here — a packet is as large
//! as a slot allows, so there is no spill, and one ring joins two sides, so
//! `tag` carries no sender.
//!
//! ## A slot in two places
//!
//! A slot is a descriptor of four words and a payload of `slot_bytes`, and the
//! two lie apart because one is sixteen bytes and the other is thousands: the
//! descriptors are a header of eight u32 words and `capacity` of them, and the
//! payloads are `capacity` runs of `slot_bytes` **elsewhere in the memory**. Which is how every ring this is modelled on
//! is built: virtio's descriptor table, avail and used are three separate
//! areas, and AF_XDP's rings are one mapping and its UMEM another. The
//! descriptors are kilobytes and the payloads are megabytes, so tying the two
//! into one block makes the small one as coarse as the large one.
//!
//!   0  capacity     a power of two
//!   1  head         position of the next message to consume
//!   2  tail         position of the next message to produce
//!   3  slot_bytes   a power of two, and a multiple of 8
//!   4  slot_base    where the payloads begin
//!   5…7 reserved
//!
//! ## One payload a slot
//!
//! Message `n` takes slot `n & (capacity - 1)` and writes its packet into that
//! slot's payload — `entry_at` and `slot_at` answer the same `position`. A
//! slot holds one live message, so its payload is free exactly when it is, and
//! there is no second bookkeeping to keep: no cursor over a shared area, no
//! rewind, no moment at which the producer has to find the ring empty before
//! it can reuse space. AF_XDP's UMEM is the same correspondence — a descriptor
//! and the buffer it names.
//!
//! What that buys is one failure mode. A push fails when the ring is full
//! (EAGAIN) and for no other reason, so the producer waits and nothing is
//! dropped. A packet larger than a slot is EMSGSIZE and is refused the same
//! way whatever the ring's state — where it once depended on how much of a
//! shared area happened to be left, so the same packet went through or did not
//! depending on what had been sent before it.
//!
//! Positions only grow and are taken modulo the capacity when read, so
//! `tail - head` is what waits. Each side writes only its own words — the
//! producer the tail and its payloads, the consumer the head — so the two
//! share the ring without a lock. A packet read by the consumer is valid
//! until it advances past the message.
//!
//! The doorbell — how the consumer is told there is something to read — is
//! not part of the ring; it is whatever call the two sides already take
//! turns at.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

use super::frame::{cmd_of, cmd_word, flags_of, FRAME_CMD, FRAME_PACKET, FRAME_RESULT, FRAME_TAG, FRAME_WORDS};

/// The address `off` bytes into a ring's memory, as a pointer to read or
/// write there.
///
/// The arithmetic is on the address and not on the pointer, which matters: a
/// ring's memory is a machine's — a wasm module's linear memory, addressed
/// from 0 — so its base is a number and not a pointer into an allocation, and
/// `ptr::add` on one of those is undefined. A release build took the licence:
/// it dropped a ring's capacity word and left the rest of the header standing,
/// so the ring read back as one of capacity 0.
fn at_of<T>(mem: *mut u8, off: usize) -> *mut T {
    (mem as usize + off) as *mut T
}

pub const RING_HEADER_WORDS: u32 = 8;

/// Bytes the descriptors of a ring of `capacity` messages take.
pub const fn ring_bytes(capacity: u32) -> u32 {
    (RING_HEADER_WORDS + capacity * FRAME_WORDS) * 4
}

/// Bytes the payloads of a ring of `capacity` slots of `slot_bytes` take.
pub const fn slots_bytes(capacity: u32, slot_bytes: u32) -> u32 {
    capacity * slot_bytes
}

/// A packet's bytes: its 16-byte header, then the payload its second word
/// names, to the word.
fn packet_bytes(u8: &[u8], at: usize) -> u64 {
    let payload = u32::from_le_bytes([u8[at + 8], u8[at + 9], u8[at + 10], u8[at + 11]]);
    (16 + payload as u64).div_ceil(8) * 8
}

#[derive(Clone)]
pub struct Ring {
    /// The memory both sides see. A ring names where it lies in it, as
    /// `mem`'s blocks do: the memory outlives every ring over it, and two
    /// rings share it.
    mem: *mut u8,
    /// Where the header and the messages start; message `slot` starts at
    /// `entry_at(slot)` words past it.
    at: u32,
    pub capacity: u32,
    pub slot_bytes: u32,
    mask: u32,
    /// Where the payloads begin, for the packet word to name one.
    slot_base: u32,
}

impl Ring {
    /// A ring already laid out: its descriptors at byte `at` of `mem`, its
    /// payloads where its header says.
    ///
    /// # Safety
    /// `mem` must stay valid, and never move, for as long as the ring is used.
    pub unsafe fn new(mem: *mut u8, at: u32) -> Ring {
        let head = unsafe { core::slice::from_raw_parts(at_of::<u32>(mem, at as usize), RING_HEADER_WORDS as usize) };
        let capacity = head[0];
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} at {at} is not a power of two");
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

    /// The header and the messages, as words.
    #[allow(clippy::mut_from_ref)]
    pub fn words(&self) -> &mut [u32] {
        // SAFETY: the memory the ring was given, which outlives it. The two
        // sides write only their own words, which is what makes the aliasing
        // sound.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u32>(self.mem, self.at as usize), (RING_HEADER_WORDS + self.capacity * FRAME_WORDS) as usize) }
    }

    /// The payloads, `capacity` of them, wherever in the memory they were put.
    #[allow(clippy::mut_from_ref)]
    fn slots(&self) -> &mut [u8] {
        // SAFETY: as `words`.
        unsafe { core::slice::from_raw_parts_mut(at_of::<u8>(self.mem, self.slot_base as usize), slots_bytes(self.capacity, self.slot_bytes) as usize) }
    }

    /// Lays out an empty ring: `capacity` descriptors at byte `at` of `mem`,
    /// and as many payloads of `slot_bytes` at `slot_base`.
    ///
    /// # Safety
    /// As `new`.
    pub unsafe fn init(mem: *mut u8, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32) -> Ring {
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} is not a power of two");
        }
        if slot_bytes == 0 || (slot_bytes & (slot_bytes - 1)) != 0 || !slot_bytes.is_multiple_of(8) {
            panic!("ring: a slot of {slot_bytes} bytes is not a power of two of whole words");
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

    /// Messages waiting to be consumed.
    pub fn len(&self) -> u32 {
        self.words()[2].wrapping_sub(self.words()[1])
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Where the message at `position` starts in `words`.
    pub fn entry_at(&self, position: u32) -> u32 {
        RING_HEADER_WORDS + (position & self.mask) * FRAME_WORDS
    }

    /// Where the payload of the message at `position` starts, the pair of
    /// `entry_at`: one a slot, so it is free exactly when the slot is.
    fn slot_at(&self, position: u32) -> usize {
        ((position & self.mask) * self.slot_bytes) as usize
    }

    // ---- producer ----

    /// Writes one message at the tail, its packet copied into the payload of
    /// the slot it takes. EAGAIN, and nothing written, when the ring is full;
    /// EMSGSIZE when the packet is larger than a slot.
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

    /// Writes one message at the tail, its packet written in place by `write`:
    /// handed the whole payload of the slot it takes, it answers the packet's
    /// length, or none when the packet did not fit. EAGAIN, and nothing
    /// written, when the ring is full; EMSGSIZE when it did not fit a slot —
    /// which does not depend on what was sent before it.
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

    // ---- consumer ----

    /// Where the message at the head starts in `words`, or -1 when empty.
    /// Read it in place, then `advance`.
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

    /// The message's packet, a view into its slot valid until `advance`, or
    /// none when it has none. EBADMSG when it names bytes outside a slot.
    ///
    /// The slot is the memory `new` was given, whose lifetime is that
    /// constructor's contract rather than this borrow's: a reader holds the
    /// packet while it acts on the ring.
    pub fn packet(&self, at: u32) -> Result<Option<&'static [u8]>, MarshalError> {
        let packet_at = self.words()[(at + FRAME_PACKET) as usize];
        if packet_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG, Some(&format!("ring: message packet at {packet_at} lies outside a slot")), None);
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

    /// Consumes the message at the head.
    pub fn advance(&self) {
        if self.is_empty() {
            panic!("ring: advance on an empty ring");
        }
        self.words()[1] = self.words()[1].wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
