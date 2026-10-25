// xpute-guest/ipc/stream.rs

//! A turn's calls from guest to host as records, run whole by the host after
//! the turn returns, so a record from turn N is done when N + 1 rises (a
//! ring gives no such guarantee).
//!
//!   0      END      words of records past the head, set at the turn's end
//!   1…     the user's
//!   head…  op, n, n words; …   bytes go as their count, then padded to a word

use crate::mem::Memory;

/// A stream word's bytes.
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

pub struct Stream {
    mem: Memory,
    at: usize,
    words: usize,
    head: usize,
    len: usize,
    /// Where an open transaction began, and whether a record past the range
    /// has dropped it.
    mark: Option<usize>,
    spilled: bool,
}

impl Stream {
    /// The word that holds how many words of records the turn published.
    pub const END: usize = 0;

    /// The `words` at `at` in `mem`. A new stream has published nothing,
    /// whatever the range held.
    ///
    /// # Safety
    /// The range stays valid, unmoved and the stream's alone (the host reads
    /// it between turns) for as long as the stream is used.
    pub unsafe fn new(mem: Memory, at: usize, words: usize, head: usize) -> Stream {
        xpute_kit::ensure!(head > Stream::END && head < words, EINVAL);
        let s = Stream {
            mem,
            at,
            words,
            head,
            len: 0,
            mark: None,
            spilled: false,
        };
        s.words()[Stream::END] = 0;
        s
    }

    #[allow(clippy::mut_from_ref)]
    fn words(&self) -> &mut [u32] {
        // SAFETY: the range `new` was given, as its contract requires.
        unsafe { core::slice::from_raw_parts_mut(self.mem.at::<u32>(self.at), self.words) }
    }

    /// Where the stream begins in its memory: its head's first word, `END`.
    pub fn head_at(&self) -> usize {
        self.at
    }

    pub fn left(&self) -> usize {
        (self.words - self.head - self.len) * WORD
    }

    pub fn publish(&mut self) {
        self.words()[Stream::END] = self.len as u32;
        self.len = 0;
    }

    /// Whether records of `bytes`, as their writers measure them
    /// (`record_bytes`, xpute-gpu encode's `call::*_bytes`), would fit whole
    /// with `leave` bytes still left after them.
    pub fn fits(&self, bytes: usize, leave: usize) -> bool {
        self.left() >= bytes + leave
    }

    /// A bug past the range reports its caller, which names the stream and the record.
    #[track_caller]
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.head + self.len;
        xpute_kit::ensure!(at + 2 + n <= self.words, ENOSPC, self.words * 4);
        self.len += 2 + n;
        let w = &mut self.words()[at..at + 2 + n];
        w[0] = op;
        w[1] = n as u32;
        &mut w[2..]
    }

    /// The records from here to `commit` are one: one that does not fit drops
    /// them all, and those after it, where outside a transaction it is a bug.
    pub fn begin(&mut self) {
        self.mark = Some(self.len);
        self.spilled = false;
    }

    /// False where the transaction did not fit: the stream is as at `begin`.
    pub fn commit(&mut self) -> bool {
        let fit = !self.spilled;
        if let Some(mark) = self.mark.take().filter(|_| !fit) {
            self.len = mark;
        }
        self.spilled = false;
        fit
    }

    /// A record of `n` words for its writer to fill; none where an open
    /// transaction no longer fits it, which drops the transaction, as a
    /// record past the range does.
    #[track_caller]
    pub fn reserve(&mut self, op: u32, n: usize) -> Option<&mut [u32]> {
        if self.mark.is_some() && (self.spilled || self.left() < (2 + n) * WORD) {
            self.spilled = true;
            return None;
        }
        Some(self.open(op, n))
    }

    #[track_caller]
    pub fn record(&mut self, op: u32, args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
        let n = record_words(args.len() + tail.len(), bytes.map(<[u8]>::len)) - 2;
        let Some(w) = self.reserve(op, n) else { return };
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
            unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), dst.as_mut_ptr().cast::<u8>(), b.len()) };
        }
    }
}

#[cfg(test)]
#[path = "stream.test.rs"]
mod test;
