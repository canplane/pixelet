//! A single-producer, single-consumer ring of fixed-size entries in linear
//! memory: the submission and completion rings between the glue and the
//! core (issue/20260913-boundaries-commands-and-what-comes-back-from-april.md,
//! "Rings"), @xpute/core/ipc/ring.ts on the other side.
//!
//! A ring is a header of four u32 words and then `capacity` entries of
//! ENTRY_WORDS words:
//!
//!   0  capacity   a power of two
//!   1  head       position of the next entry to consume
//!   2  tail       position of the next entry to produce
//!   3  reserved
//!
//! Positions only grow and are taken modulo the capacity when read, so
//! `tail - head` is the number waiting and a full ring is one where that is
//! the capacity. Each side writes only its own word: the producer the tail,
//! the consumer the head — which is what lets the two share the ring without
//! a lock. Both run on one thread today, taking turns at the doorbell; the
//! discipline is what keeps that true when they do not.
//!
//! An entry, both directions, mirrors the first words of a frame
//! (@xpute/core/ipc/frame.ts):
//!
//!   0  user       the submitter's, echoed back on the completion
//!   1  op | flags op in the low 16 bits
//!   2  result     on a completion: a value, or a negative errno
//!   3  reserved
//!   4… args       twelve words; anything larger is a block in the memory,
//!                 named by its address
//!
//! A full submission ring refuses (EAGAIN) and the submitter waits; a full
//! completion ring stops the consumer draining submissions, so nothing is
//! ever dropped in either direction.

pub const HEADER_WORDS: usize = 4;
pub const ENTRY_WORDS: usize = 16;
pub const ARG_WORDS: usize = ENTRY_WORDS - 4;

pub type Entry = [u32; ENTRY_WORDS];

pub const USER: usize = 0;
pub const OP: usize = 1;
pub const RESULT: usize = 2;
pub const ARGS: usize = 4;

/// Words a ring of `capacity` entries takes.
pub const fn ring_words(capacity: usize) -> usize {
    HEADER_WORDS + capacity * ENTRY_WORDS
}

/// A ring laid out at an address. Holds nothing of its own: every read and
/// write goes to the memory, where the other side sees it.
#[derive(Clone, Copy)]
pub struct Ring {
    at: *mut u32,
}

impl Ring {
    /// Lays out an empty ring of `capacity` entries at `at`.
    ///
    /// # Safety
    /// `at` must address `ring_words(capacity)` words, 4-aligned, that
    /// nothing else writes but the ring's two sides.
    pub unsafe fn init(at: *mut u32, capacity: u32) -> Ring {
        assert!(capacity.is_power_of_two(), "a ring's capacity is a power of two");
        core::ptr::write_bytes(at, 0, ring_words(capacity as usize));
        *at = capacity;
        Ring { at }
    }

    /// A ring already laid out at `at`.
    ///
    /// # Safety
    /// As for `init`, and a ring must be there.
    pub unsafe fn at(at: *mut u32) -> Ring {
        Ring { at }
    }

    pub fn address(&self) -> usize {
        self.at as usize
    }

    fn word(&self, i: usize) -> u32 {
        // SAFETY: within the header, which `init` laid out.
        unsafe { *self.at.add(i) }
    }

    fn set_word(&self, i: usize, v: u32) {
        // SAFETY: as above.
        unsafe { *self.at.add(i) = v }
    }

    pub fn capacity(&self) -> u32 {
        self.word(0)
    }

    pub fn head(&self) -> u32 {
        self.word(1)
    }

    pub fn tail(&self) -> u32 {
        self.word(2)
    }

    /// Entries waiting to be consumed.
    pub fn len(&self) -> u32 {
        self.tail().wrapping_sub(self.head())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity()
    }

    fn entry(&self, position: u32) -> *mut u32 {
        let slot = (position & (self.capacity() - 1)) as usize;
        // SAFETY: slot < capacity, inside what `init` laid out.
        unsafe { self.at.add(HEADER_WORDS + slot * ENTRY_WORDS) }
    }

    // ---- producer ----

    /// Writes one entry at the tail. False, and nothing written, when full.
    pub fn push(&self, e: &Entry) -> bool {
        if self.is_full() {
            return false;
        }
        let tail = self.tail();
        // SAFETY: the slot is the producer's until the tail moves past it.
        unsafe { core::ptr::copy_nonoverlapping(e.as_ptr(), self.entry(tail), ENTRY_WORDS) };
        self.set_word(2, tail.wrapping_add(1));
        true
    }

    // ---- consumer ----

    /// The entry at the head, copied out, or none when empty.
    pub fn peek(&self) -> Option<Entry> {
        if self.is_empty() {
            return None;
        }
        let mut e = [0u32; ENTRY_WORDS];
        // SAFETY: the producer finished the slot before moving the tail.
        unsafe { core::ptr::copy_nonoverlapping(self.entry(self.head()), e.as_mut_ptr(), ENTRY_WORDS) };
        Some(e)
    }

    /// Consumes the entry at the head.
    pub fn advance(&self) {
        assert!(!self.is_empty(), "advance on an empty ring");
        self.set_word(1, self.head().wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(user: u32) -> Entry {
        let mut e = [0; ENTRY_WORDS];
        e[USER] = user;
        e[ARGS] = user * 10;
        e
    }

    #[test]
    fn entries_come_out_in_order_across_the_wrap_and_a_full_ring_refuses() {
        let mut mem = vec![0u32; ring_words(4)];
        // SAFETY: the vector is the ring's words.
        let r = unsafe { Ring::init(mem.as_mut_ptr(), 4) };
        for round in 0..3u32 {
            for k in 0..4 {
                assert!(r.push(&entry(round * 4 + k)));
            }
            assert!(r.is_full());
            assert!(!r.push(&entry(99)), "a full ring refuses");
            for k in 0..4 {
                let e = r.peek().expect("an entry waits");
                assert_eq!((e[USER], e[ARGS]), (round * 4 + k, (round * 4 + k) * 10));
                r.advance();
            }
            assert!(r.is_empty() && r.peek().is_none());
        }
        assert_eq!((r.head(), r.tail()), (12, 12), "positions only grow");
    }
}
