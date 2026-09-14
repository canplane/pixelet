// xpute_core/ipc/ring.rs

//! A single-producer, single-consumer ring of fixed-size entries in a buffer
//! both sides can see — the submission and completion rings between a glue
//! and its core, laid out once and shared by address.
//!
//! A ring is a header of four u32 words and then `capacity` entries of
//! RING_ENTRY_WORDS words:
//!
//!   0  capacity   a power of two
//!   1  head       position of the next entry to consume
//!   2  tail       position of the next entry to produce
//!   3  reserved
//!
//! Positions only grow and are taken modulo the capacity when read, so
//! `tail - head` is what waits and a full ring is one where that is the
//! capacity. Each side writes only its own word — the producer the tail, the
//! consumer the head — so the two share the ring without a lock.
//!
//! An entry mirrors the first words of a frame (frame.rs):
//!
//!   0  user       the submitter's, echoed back on the completion
//!   1  op | flags op in the low 16 bits
//!   2  result     on a completion: a value, or a negative Errno
//!   3  reserved
//!   4… args       twelve words; anything larger is a block in the buffer,
//!                 named by its offset
//!
//! A full ring refuses a push (EAGAIN): the producer waits, and nothing is
//! dropped. The doorbell — how the consumer is told there is something to
//! read — is not part of the ring; it is whatever call the two sides already
//! take turns at.

use crate::kit::array::{ArrayBuffer, TypedArray};
use crate::sys::errno::Errno;

pub const RING_HEADER_WORDS: u32 = 4;
pub const RING_ENTRY_WORDS: u32 = 16;
pub const RING_ARG_WORDS: u32 = RING_ENTRY_WORDS - 4;

pub const RING_USER: u32 = 0;
pub const RING_OP: u32 = 1;
pub const RING_RESULT: u32 = 2;
pub const RING_ARGS: u32 = 4;

/// One entry's words, as the core's side writes a completion whole.
pub type Entry = [u32; RING_ENTRY_WORDS as usize];

/// Words a ring of `capacity` entries takes.
pub fn ring_words(capacity: u32) -> u32 {
    RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS
}

#[derive(Clone)]
pub struct Ring {
    /// The whole ring's words; entry `slot` starts at `entry_at(slot)`.
    pub words: TypedArray<u32>,
    pub capacity: u32,
    mask: u32,
}

impl Ring {
    /// A ring already laid out at byte `offset` of `buffer`.
    pub fn new(buffer: ArrayBuffer, offset: u32) -> Ring {
        let capacity = TypedArray::<u32>::over(buffer.clone(), offset as usize, 1)[0];
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} at {offset} is not a power of two");
        }
        Ring {
            words: TypedArray::over(buffer, offset as usize, ring_words(capacity) as usize),
            capacity,
            mask: capacity - 1,
        }
    }

    /// Lays out an empty ring of `capacity` entries at byte `offset`.
    pub fn init(buffer: ArrayBuffer, offset: u32, capacity: u32) -> Ring {
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            panic!("ring: capacity {capacity} is not a power of two");
        }
        let words = TypedArray::<u32>::over(buffer.clone(), offset as usize, ring_words(capacity) as usize);
        words.fill(0);
        words.put(0, capacity);
        Ring::new(buffer, offset)
    }

    pub fn head(&self) -> u32 {
        self.words[1]
    }

    pub fn tail(&self) -> u32 {
        self.words[2]
    }

    /// Entries waiting to be consumed.
    pub fn len(&self) -> u32 {
        self.words[2].wrapping_sub(self.words[1])
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Where the entry at `position` starts in `words`.
    pub fn entry_at(&self, position: u32) -> u32 {
        RING_HEADER_WORDS + (position & self.mask) * RING_ENTRY_WORDS
    }

    // ---- producer ----

    /// Writes one entry at the tail: `user`, `op`, and up to twelve args.
    /// EAGAIN, and nothing written, when full.
    pub fn push(&self, user: u32, op: u32, args: &[u32]) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        if args.len() > RING_ARG_WORDS as usize {
            panic!("ring: {} args, at most {RING_ARG_WORDS}", args.len());
        }
        let tail = self.words[2];
        let at = self.entry_at(tail) as usize;
        let w = self.words.as_mut_slice();
        w[at..at + RING_ENTRY_WORDS as usize].fill(0);
        w[at + RING_USER as usize] = user;
        w[at + RING_OP as usize] = op;
        for (k, &arg) in args.iter().enumerate() {
            w[at + RING_ARGS as usize + k] = arg;
        }
        w[2] = tail.wrapping_add(1);
        Errno::OK
    }

    /// Writes one entry at the tail whole — a completion, with its result.
    /// EAGAIN, and nothing written, when full.
    pub fn push_entry(&self, entry: &Entry) -> Errno {
        if self.len() >= self.capacity {
            return Errno::EAGAIN;
        }
        let tail = self.words[2];
        let at = self.entry_at(tail) as usize;
        let w = self.words.as_mut_slice();
        w[at..at + RING_ENTRY_WORDS as usize].copy_from_slice(entry);
        w[2] = tail.wrapping_add(1);
        Errno::OK
    }

    // ---- consumer ----

    /// Where the entry at the head starts in `words`, or -1 when empty. Read
    /// it in place, then `advance`.
    pub fn peek(&self) -> i64 {
        if self.is_empty() {
            -1
        } else {
            self.entry_at(self.words[1]) as i64
        }
    }

    /// Consumes the entry at the head.
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
