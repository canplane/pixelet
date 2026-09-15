// xpute_core/ipc/ring.rs

//! A single-producer, single-consumer ring of messages in a buffer both
//! sides can see — the submission and completion rings between a shell and
//! its kernel, laid out once and shared by address.
//!
//! A message is what a frame (frame.rs) needs once the transport is shared
//! memory rather than transfer: no spill, since a packet is as large as the
//! arena allows; no args/body split, since the packet is a tree and a large
//! body is a leaf of it; no worker id, since one ring joins two sides. What
//! is left is four words:
//!
//!   0  tag          the submitter's, echoed on the reply; on a signal, its
//!                   subject
//!   1  cmd | flags  cmd (cmd.rs: major << 8 | minor) in the low 16 bits,
//!                   FrameFlag in the high 16
//!   2  result       on a reply: a value, or a negative Errno; else 0
//!   3  packet       where the message's XTP packet starts, from the ring's
//!                   own start, or 0 for none — its length is in its header
//!
//! On the completion ring, `RES` tells a reply from a signal the kernel
//! raised itself. `ACKREQ` on a submission asks for exactly one reply.
//!
//! Layout: a header of eight u32 words, `capacity` messages, then the arena
//! the producer writes packets into, 8-aligned, one after another:
//!
//!   0  capacity     a power of two
//!   1  head         position of the next message to consume
//!   2  tail         position of the next message to produce
//!   3  arena_bytes  a multiple of 8
//!   4  arena_tail   where the producer's next packet goes, in the arena
//!   5…7 reserved
//!
//! Positions only grow and are taken modulo the capacity when read, so
//! `tail - head` is what waits. Each side writes only its own words — the
//! producer the tail and the arena, the consumer the head — so the two share
//! the ring without a lock. The producer rewinds the arena when it finds the
//! ring empty: every packet written before then belongs to a message already
//! consumed. A packet read by the consumer is valid until it advances past
//! the message.
//!
//! A full ring, or an arena without room, refuses a push (EAGAIN): the
//! producer waits, and nothing is dropped. A packet larger than the whole
//! arena can never fit and is refused as EMSGSIZE. The doorbell — how the
//! consumer is told there is something to read — is not part of the ring;
//! it is whatever call the two sides already take turns at.

use crate::kit::array::{ArrayBuffer, TypedArray, U8Array};
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

pub const RING_HEADER_WORDS: u32 = 8;
pub const RING_ENTRY_WORDS: u32 = 4;

pub const RING_TAG: u32 = 0;
pub const RING_CMD: u32 = 1;
pub const RING_RESULT: u32 = 2;
pub const RING_PACKET: u32 = 3;

/// Bytes a ring of `capacity` messages and an arena of `arena_bytes` takes.
pub fn ring_bytes(capacity: u32, arena_bytes: u32) -> u32 {
    (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) * 4 + arena_bytes
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
    pub arena_bytes: u32,
    mask: u32,
    u8: U8Array,
    /// Where the arena starts, from the ring's start.
    arena_at: u32,
}

impl Ring {
    /// A ring already laid out at byte `offset` of `buffer`.
    pub fn new(buffer: ArrayBuffer, offset: u32) -> Ring {
        let head = TypedArray::<u32>::over(buffer.clone(), offset as usize, RING_HEADER_WORDS as usize);
        let capacity = head[0];
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} at {offset} is not a power of two");
        }
        let arena_bytes = head[3];
        let words = TypedArray::<u32>::over(buffer.clone(), offset as usize, (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) as usize);
        let arena_at = (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) * 4;
        let u8 = TypedArray::<u8>::over(buffer, offset as usize, ring_bytes(capacity, arena_bytes) as usize);
        Ring {
            words,
            capacity,
            arena_bytes,
            mask: capacity - 1,
            u8,
            arena_at,
        }
    }

    /// Lays out an empty ring of `capacity` messages and an arena of
    /// `arena_bytes` at byte `offset`.
    pub fn init(buffer: ArrayBuffer, offset: u32, capacity: u32, arena_bytes: u32) -> Ring {
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} is not a power of two");
        }
        if !arena_bytes.is_multiple_of(8) {
            panic!("ring: an arena of {arena_bytes} bytes is not whole words");
        }
        TypedArray::<u8>::over(buffer.clone(), offset as usize, ring_bytes(capacity, arena_bytes) as usize).fill(0);
        let head = TypedArray::<u32>::over(buffer.clone(), offset as usize, RING_HEADER_WORDS as usize);
        head.put(0, capacity);
        head.put(3, arena_bytes);
        Ring::new(buffer, offset)
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

    // ---- producer ----

    /// Writes one message at the tail, its packet copied into the arena.
    /// EAGAIN, and nothing written, when the ring is full or the arena has no
    /// room left; EMSGSIZE when the packet is larger than the whole arena.
    pub fn push(&self, tag: u32, cmd: u32, flags: u32, packet: Option<&U8Array>, result: i32) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words.as_mut_slice();
        if w[2] == w[1] {
            w[4] = 0;
        }
        let mut packet_at = 0;
        if let Some(packet) = packet {
            let need = packet.byte_length().div_ceil(8) * 8;
            if need > self.arena_bytes as usize {
                return Errno::EMSGSIZE;
            }
            if w[4] as usize + need > self.arena_bytes as usize {
                return Errno::EAGAIN;
            }
            packet_at = self.arena_at + w[4];
            let at = packet_at as usize;
            let bytes = self.u8.as_mut_slice();
            bytes[at..at + packet.byte_length()].copy_from_slice(packet.as_slice());
            bytes[at + packet.byte_length()..at + need].fill(0);
            w[4] += need as u32;
        }
        let at = self.entry_at(w[2]) as usize;
        w[at + RING_TAG as usize] = tag;
        w[at + RING_CMD as usize] = (cmd & 0xffff) | ((flags & 0xffff) << 16);
        w[at + RING_RESULT as usize] = result as u32;
        w[at + RING_PACKET as usize] = packet_at;
        w[2] = w[2].wrapping_add(1);
        Errno::OK
    }

    /// Writes one message at the tail, its packet written in place by `write`:
    /// handed what is left of the arena, it answers the packet's length, or
    /// none when the packet did not fit. EAGAIN, and nothing written, when the
    /// ring is full or the packet did not fit what is left; EMSGSIZE when it
    /// did not fit an arena with nothing in it.
    pub fn push_in_place(&self, tag: u32, cmd: u32, flags: u32, result: i32, write: impl FnOnce(&mut [u8]) -> Option<u32>) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let w = self.words.as_mut_slice();
        if w[2] == w[1] {
            w[4] = 0;
        }
        let start = (self.arena_at + w[4]) as usize;
        let end = (self.arena_at + self.arena_bytes) as usize;
        let bytes = self.u8.as_mut_slice();
        let Some(n) = write(&mut bytes[start..end]) else {
            return if w[4] == 0 { Errno::EMSGSIZE } else { Errno::EAGAIN };
        };
        let need = (n as usize).div_ceil(8) * 8;
        bytes[start + n as usize..start + need].fill(0);
        w[4] += need as u32;
        let at = self.entry_at(w[2]) as usize;
        w[at + RING_TAG as usize] = tag;
        w[at + RING_CMD as usize] = (cmd & 0xffff) | ((flags & 0xffff) << 16);
        w[at + RING_RESULT as usize] = result as u32;
        w[at + RING_PACKET as usize] = start as u32;
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

    /// The message's packet, a view into the arena valid until `advance`, or
    /// none when it has none. EBADMSG when it names bytes outside the arena.
    pub fn packet(&self, at: u32) -> Result<Option<U8Array>, MarshalError> {
        let packet_at = self.words[(at + RING_PACKET) as usize];
        if packet_at == 0 {
            return Ok(None);
        }
        let end = self.arena_at as u64 + self.arena_bytes as u64;
        let p = packet_at as u64;
        if packet_at < self.arena_at || p + 16 > end || packet_bytes(&self.u8, packet_at as usize) > end - p {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("ring: message packet at {packet_at} lies outside the arena")), None));
        }
        let bytes = packet_bytes(&self.u8, packet_at as usize) as usize;
        Ok(Some(self.u8.subarray(packet_at as usize, packet_at as usize + bytes)))
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
