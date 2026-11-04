// xpute-guest/ipc/stream.rs

//! A turn's calls from guest to host as records, published at the turn's
//! end and run whole by the host.
//!
//!   0      END      words of records past the head, published at the turn's end
//!   1      HEAD     words before the first record, set as it is made
//!   2…     the user's
//!   head…  op, n, n words; …   bytes go as their count, then padded to a word
//!
//! A host reads where the records begin from the stream itself (`HEAD`).
//!
//! END hands the records from guest to host and back, by the rules a ring's
//! position words keep (ring.rs), whether the two take turns on one thread
//! or run at once on two:
//!
//! - R1. END has one writer at a time: the guest stores a count into it only
//!   while it reads 0, and the host stores 0 into it only while it reads a
//!   count.
//! - R2. END is stored with release and loaded with acquire. The guest writes
//!   its records, then stores their count; the host loads the count before
//!   it reads them, and stores 0 once it is done with them; the guest loads
//!   END before it writes records again.
//! - R3. The records, and the memory a record names for the host to read,
//!   are the host's from the count's store until the host stores 0. A host
//!   stores 0 whether or not it ran them, so a host with nothing to run them
//!   on never holds the guest.
//!
//! A guest that finds END not yet 0 writes no records (`is_free`): the turn
//! leaves what it would have recorded for a later one.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::mem::Memory;

pub const WORD: usize = size_of::<u32>();

/// A record's words: its op and count, `args`, and `bytes` as their count then words.
pub const fn record_words(args: usize, bytes: Option<usize>) -> usize {
    2 + args
        + match bytes {
            Some(b) => 1 + b.div_ceil(WORD),
            None => 0,
        }
}

/// A record's size as a stream's room is counted.
pub const fn record_bytes(args: usize, bytes: Option<usize>) -> usize {
    record_words(args, bytes) * WORD
}

/// Whether a transaction is open, where it began, and whether a record that
/// did not fit has dropped it.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Off,
    Open(usize),
    Spilled(usize),
}

pub struct Stream {
    mem: Memory,
    at: usize,
    words: usize,
    head: usize,
    len: usize,
    mark: Mark,
    /// The host stored END back to 0 since the last publish, as last loaded.
    free: bool,
}

impl Stream {
    pub const END: usize = 0;
    pub const HEAD: usize = 1;
    /// The stream's own words, before the user's.
    pub const OWN_WORDS: usize = 2;

    /// The `words` at `at` in `mem`, word-aligned. A new stream has published
    /// nothing, whatever the range held.
    ///
    /// # Safety
    /// The range stays valid, unmoved and the stream's alone, the host
    /// reading it only while END says so, for as long as the stream is used;
    /// no host runs it before `new` returns.
    pub unsafe fn new(mem: Memory, at: usize, words: usize, head: usize) -> Stream {
        xpute_kit::ensure!(head >= Stream::OWN_WORDS && head < words, EINVAL);
        xpute_kit::ensure!(mem.at::<u32>(at).is_aligned(), EINVAL, at);
        let s = Stream {
            mem,
            at,
            words,
            head,
            len: 0,
            mark: Mark::Off,
            free: true,
        };
        s.end().store(0, Ordering::Release);
        s.words(Stream::HEAD, 1)[0] = head as u32;
        s
    }

    /// END, only ever touched as an atomic (R2).
    fn end(&self) -> &AtomicU32 {
        // SAFETY: the range's first word, aligned as `new` checked.
        unsafe { AtomicU32::from_ptr(self.mem.at::<u32>(self.at)) }
    }

    /// The `n` words from word `k`, past END: the guest's while the stream
    /// is free.
    #[allow(clippy::mut_from_ref)]
    fn words(&self, k: usize, n: usize) -> &mut [u32] {
        xpute_kit::ensure!(k > Stream::END && k + n <= self.words, ENOSPC, k, n);
        // SAFETY: inside the range `new` was given, as its contract requires,
        // END left out; while the stream is free nothing else reads it.
        unsafe { core::slice::from_raw_parts_mut(self.mem.at::<u32>(self.at + k * WORD), n) }
    }

    /// Where the stream's words begin in its memory, `END` first.
    pub fn head_at(&self) -> usize {
        self.at
    }

    pub fn left(&self) -> usize {
        (self.words - self.head - self.len) * WORD
    }

    /// Whether the host has stored END back to 0 since the last publish:
    /// the stream is the guest's to write (R1, R3).
    pub fn is_free(&mut self) -> bool {
        if !self.free {
            self.free = self.end().load(Ordering::Acquire) == 0;
        }
        self.free
    }

    /// The turn's records handed to the host (R2); a turn that recorded
    /// none leaves END as it is.
    pub fn publish(&mut self) {
        if self.len == 0 {
            return;
        }
        self.end().store(self.len as u32, Ordering::Release);
        self.len = 0;
        self.free = false;
    }

    /// Whether records of `bytes` (`record_bytes`, xpute-gpu encode's
    /// `call::*_bytes`) fit with `leave` bytes left after them.
    pub fn fits(&self, bytes: usize, leave: usize) -> bool {
        self.left() >= bytes + leave
    }

    /// A bug past the range, or while the host holds the stream (EBUSY),
    /// reports its caller, which names the stream and the record.
    #[track_caller]
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.head + self.len;
        xpute_kit::ensure!(at + 2 + n <= self.words, ENOSPC, self.words * 4);
        xpute_kit::ensure!(self.is_free(), EBUSY, self.at);
        self.len += 2 + n;
        let w = self.words(at, 2 + n);
        w[0] = op;
        w[1] = n as u32;
        &mut w[2..]
    }

    /// The records from here to `commit` are one: one that does not fit drops
    /// them all, and those after it, where outside a transaction it is a bug.
    pub fn begin(&mut self) {
        self.mark = Mark::Open(self.len);
    }

    /// False where the transaction did not fit: the stream is as at `begin`.
    pub fn commit(&mut self) -> bool {
        let mark = core::mem::replace(&mut self.mark, Mark::Off);
        if let Mark::Spilled(at) = mark {
            self.len = at;
        }
        !matches!(mark, Mark::Spilled(_))
    }

    /// A record of `n` words for its writer to fill; none where an open
    /// transaction no longer fits it, which drops the transaction.
    #[track_caller]
    pub fn reserve(&mut self, op: u32, n: usize) -> Option<&mut [u32]> {
        match self.mark {
            Mark::Spilled(_) => return None,
            Mark::Open(at) if self.left() < (2 + n) * WORD => {
                self.mark = Mark::Spilled(at);
                return None;
            }
            _ => {}
        }
        Some(self.open(op, n))
    }

    #[track_caller]
    pub fn record(&mut self, op: u32, args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
        let n = record_words(args.len() + tail.len(), bytes.map(<[u8]>::len)) - 2;
        let Some(w) = self.reserve(op, n) else { return };
        fill(w, args, tail, bytes);
    }
}

/// A record's words past its op and count: `args`, `tail`, and `bytes` as
/// their count then words.
fn fill(w: &mut [u32], args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
    w[..args.len()].copy_from_slice(args);
    w[args.len()..args.len() + tail.len()].copy_from_slice(tail);
    if let Some(b) = bytes {
        let k = args.len() + tail.len();
        w[k] = b.len() as u32;
        let dst = &mut w[k + 1..];
        if let Some(last) = dst.last_mut() {
            *last = 0;
        }
        // SAFETY: the words opened, as bytes; at least `b.len()` of them.
        unsafe {
            core::ptr::copy_nonoverlapping(b.as_ptr(), dst.as_mut_ptr().cast::<u8>(), b.len())
        };
    }
}

#[cfg(test)]
#[path = "stream.test.rs"]
mod test;
