// xpute_core/ipc/ring.rs

//! A single-producer, single-consumer ring of messages in a buffer both
//! sides can see — the submission and completion rings between a shell and
//! its kernel, laid out once and shared by address.
//!
//! A message is what a frame (frame.rs) needs once the transport is shared
//! memory rather than transfer: no spill, since a packet is as large as a
//! frame allows; no args/body split, since the packet is a tree and a large
//! body is a leaf of it; no worker id, since one ring joins two sides. What
//! is left is four words:
//!
//!   0  tag          the submitter's, echoed on the reply; on a signal, its
//!                   subject
//!   1  cmd | flags  cmd (cmd.rs: major << 8 | minor) in the low 16 bits,
//!                   FrameFlag in the high 16
//!   2  result       on a reply: a value, or a negative Errno; else 0
//!   3  packet       where the message's XTP packet starts, an address in
//!                   the memory the two sides share, or 0 for none — its
//!                   length is in its header
//!
//! On the completion ring, `RES` tells a reply from a signal the kernel
//! raised itself. `ACKREQ` on a submission asks for exactly one reply.
//!
//! ## Two areas, not one
//!
//! The descriptors are a header of eight u32 words and `capacity` messages;
//! the packets live in an **arena elsewhere in the memory**, `capacity`
//! frames of `frame_bytes` each. Which is how every ring this is modelled on
//! is built: virtio's descriptor table, avail and used are three separate
//! areas, and AF_XDP's rings are one mapping and its UMEM another. The
//! descriptors are kilobytes and the arena is megabytes, so tying the two
//! into one block makes the small one as coarse as the large one.
//!
//!   0  capacity     a power of two
//!   1  head         position of the next message to consume
//!   2  tail         position of the next message to produce
//!   3  frame_bytes  a power of two, and a multiple of 8
//!   4  arena_at     where the frames begin
//!   5…7 reserved
//!
//! ## A frame a slot
//!
//! Message `n` writes its packet into frame `n & (capacity - 1)` — the frame
//! of the slot the message itself takes. A slot holds one live message, so
//! its frame is free exactly when it is, and there is no second bookkeeping
//! to keep: no arena cursor, no rewind, no moment at which the producer has
//! to find the ring empty before it can reuse space. AF_XDP's UMEM is the
//! same correspondence — a descriptor and the frame it names.
//!
//! What that buys is one failure mode. A push fails when the ring is full
//! (EAGAIN) and for no other reason, so the producer waits and nothing is
//! dropped. A packet larger than a frame is EMSGSIZE and is refused the same
//! way whatever the ring's state — where it once depended on how much of a
//! shared arena happened to be left, so the same packet went through or did
//! not depending on what had been sent before it.
//!
//! Positions only grow and are taken modulo the capacity when read, so
//! `tail - head` is what waits. Each side writes only its own words — the
//! producer the tail and its frames, the consumer the head — so the two
//! share the ring without a lock. A packet read by the consumer is valid
//! until it advances past the message.
//!
//! The doorbell — how the consumer is told there is something to read — is
//! not part of the ring; it is whatever call the two sides already take
//! turns at.

use crate::kit::array::{ArrayBuffer, TypedArray, U8Array};
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

pub const RING_HEADER_WORDS: u32 = 8;
pub const RING_ENTRY_WORDS: u32 = 4;

pub const RING_TAG: u32 = 0;
pub const RING_CMD: u32 = 1;
pub const RING_RESULT: u32 = 2;
pub const RING_PACKET: u32 = 3;

/// Bytes the descriptors of a ring of `capacity` messages take.
pub const fn ring_bytes(capacity: u32) -> u32 {
    (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) * 4
}

/// Bytes the arena of a ring of `capacity` frames of `frame_bytes` takes.
pub const fn arena_bytes(capacity: u32, frame_bytes: u32) -> u32 {
    capacity * frame_bytes
}

/// A packet's bytes: its 16-byte header, then the payload its second word
/// names, to the word.
fn packet_bytes(u8: &U8Array, at: usize) -> u64 {
    let payload = u32::from_le_bytes([u8[at + 8], u8[at + 9], u8[at + 10], u8[at + 11]]);
    (16 + payload as u64).div_ceil(8) * 8
}

#[derive(Clone)]
pub struct Ring {
    /// The header and the messages, as words; message `slot` starts at
    /// `entry_at(slot)`.
    pub words: TypedArray<u32>,
    pub capacity: u32,
    pub frame_bytes: u32,
    mask: u32,
    /// The frames, `capacity` of them, wherever in the memory they were put.
    arena: U8Array,
    /// Where the frames begin, for the packet word to name one.
    arena_at: u32,
}

impl Ring {
    /// A ring already laid out: its descriptors at byte `at` of `mem`, its
    /// arena where its header says.
    pub fn new(mem: ArrayBuffer, at: u32) -> Ring {
        let head = TypedArray::<u32>::over(mem.clone(), at as usize, RING_HEADER_WORDS as usize);
        let capacity = head[0];
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} at {at} is not a power of two");
        }
        let frame_bytes = head[3];
        let arena_at = head[4];
        let words = TypedArray::<u32>::over(mem.clone(), at as usize, (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) as usize);
        let arena = TypedArray::<u8>::over(mem, arena_at as usize, arena_bytes(capacity, frame_bytes) as usize);
        Ring {
            words,
            capacity,
            frame_bytes,
            mask: capacity - 1,
            arena,
            arena_at,
        }
    }

    /// Lays out an empty ring: `capacity` descriptors at byte `at` of `mem`,
    /// and as many frames of `frame_bytes` at `arena_at`.
    pub fn init(mem: ArrayBuffer, at: u32, capacity: u32, arena_at: u32, frame_bytes: u32) -> Ring {
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} is not a power of two");
        }
        if frame_bytes == 0 || (frame_bytes & (frame_bytes - 1)) != 0 || !frame_bytes.is_multiple_of(8) {
            panic!("ring: a frame of {frame_bytes} bytes is not a power of two of whole words");
        }
        TypedArray::<u8>::over(mem.clone(), at as usize, ring_bytes(capacity) as usize).fill(0);
        let head = TypedArray::<u32>::over(mem.clone(), at as usize, RING_HEADER_WORDS as usize);
        head.put(0, capacity);
        head.put(3, frame_bytes);
        head.put(4, arena_at);
        Ring::new(mem, at)
    }

    pub fn head(&self) -> u32 {
        self.words[1]
    }

    pub fn tail(&self) -> u32 {
        self.words[2]
    }

    /// Messages waiting to be consumed.
    pub fn len(&self) -> u32 {
        self.words[2].wrapping_sub(self.words[1])
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Where the message at `position` starts in `words`.
    pub fn entry_at(&self, position: u32) -> u32 {
        RING_HEADER_WORDS + (position & self.mask) * RING_ENTRY_WORDS
    }

    /// Where the frame of the message at `position` starts in the arena. One
    /// frame a slot, so it is free exactly when the slot is.
    fn frame_at(&self, position: u32) -> usize {
        ((position & self.mask) * self.frame_bytes) as usize
    }

    // ---- producer ----

    /// Writes one message at the tail, its packet copied into the frame of
    /// the slot it takes. EAGAIN, and nothing written, when the ring is full;
    /// EMSGSIZE when the packet is larger than a frame.
    pub fn push(&self, tag: u32, cmd: u32, flags: u32, packet: Option<&U8Array>, result: i32) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words.as_mut_slice();
        let mut packet_at = 0;
        if let Some(packet) = packet {
            let need = packet.byte_length().div_ceil(8) * 8;
            if need > self.frame_bytes as usize {
                return Errno::EMSGSIZE;
            }
            let start = self.frame_at(w[2]);
            let bytes = self.arena.as_mut_slice();
            bytes[start..start + packet.byte_length()].copy_from_slice(packet.as_slice());
            bytes[start + packet.byte_length()..start + need].fill(0);
            packet_at = self.arena_at + start as u32;
        }
        let at = self.entry_at(w[2]) as usize;
        w[at + RING_TAG as usize] = tag;
        w[at + RING_CMD as usize] = (cmd & 0xffff) | ((flags & 0xffff) << 16);
        w[at + RING_RESULT as usize] = result as u32;
        w[at + RING_PACKET as usize] = packet_at;
        w[2] = w[2].wrapping_add(1);
        Errno::OK
    }

    /// Writes one message at the tail, its packet written in place by
    /// `write`: handed the whole frame of the slot it takes, it answers the
    /// packet's length, or none when the packet did not fit one. EAGAIN, and
    /// nothing written, when the ring is full; EMSGSIZE when it did not fit a
    /// frame — which does not depend on what was sent before it.
    pub fn push_in_place(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: impl FnOnce(&mut [u8]) -> Option<u32>) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words.as_mut_slice();
        let start = self.frame_at(w[2]);
        let end = start + self.frame_bytes as usize;
        let bytes = self.arena.as_mut_slice();
        let Some(n) = write(&mut bytes[start..end]) else {
            return Errno::EMSGSIZE;
        };
        let need = (n as usize).div_ceil(8) * 8;
        bytes[start + n as usize..start + need].fill(0);
        let at = self.entry_at(w[2]) as usize;
        w[at + RING_TAG as usize] = tag;
        w[at + RING_CMD as usize] = (cmd & 0xffff) | ((flags & 0xffff) << 16);
        w[at + RING_RESULT as usize] = result as u32;
        w[at + RING_PACKET as usize] = self.arena_at + start as u32;
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
            self.entry_at(self.words[1]) as i64
        }
    }

    pub fn tag(&self, at: u32) -> u32 {
        self.words[(at + RING_TAG) as usize]
    }

    pub fn cmd(&self, at: u32) -> u32 {
        self.words[(at + RING_CMD) as usize] & 0xffff
    }

    pub fn flags(&self, at: u32) -> u32 {
        self.words[(at + RING_CMD) as usize] >> 16
    }

    pub fn result(&self, at: u32) -> i32 {
        self.words[(at + RING_RESULT) as usize] as i32
    }

    /// The message's packet, a view into its frame valid until `advance`, or
    /// none when it has none. EBADMSG when it names bytes outside a frame.
    pub fn packet(&self, at: u32) -> Result<Option<U8Array>, MarshalError> {
        let packet_at = self.words[(at + RING_PACKET) as usize];
        if packet_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG, Some(&format!("ring: message packet at {packet_at} lies outside a frame")), None);
        let off = packet_at.checked_sub(self.arena_at).ok_or_else(bad)? as usize;
        if !off.is_multiple_of(self.frame_bytes as usize) || off + 16 > self.arena.byte_length() {
            return Err(bad());
        }
        let bytes = packet_bytes(&self.arena, off);
        if bytes > self.frame_bytes as u64 {
            return Err(bad());
        }
        Ok(Some(self.arena.subarray(off, off + bytes as usize)))
    }

    /// Consumes the message at the head.
    pub fn advance(&self) {
        if self.is_empty() {
            panic!("ring: advance on an empty ring");
        }
        self.words.put(1, self.words[1].wrapping_add(1));
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
