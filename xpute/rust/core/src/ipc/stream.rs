// xpute-core/ipc/stream.rs

//! A stream carries a turn's calls from the core to the runtime as records.
//! The core publishes the records at the end of the turn, and the runtime
//! runs them all. The stream's words:
//!
//!   0      END      record words past the head, stored at the turn's end
//!   1      HEAD     words before the first record, set when the stream is made
//!   2…     the user's own words
//!   head…  records: op, n, then n words; bytes go as their count, then the
//!          bytes padded to a word
//!
//! A runtime reads where the records begin from the stream itself (`HEAD`).
//!
//! END hands the records from the core to the runtime and back. It keeps the
//! rules a ring's position words keep (ring.rs), whether the two sides take
//! turns on one thread or run at once on two:
//!
//! - R1. END has one writer at a time: the core stores a count into it only
//!   while it reads 0, and the runtime stores 0 into it only while it reads a
//!   count.
//! - R2. END is stored with release and loaded with acquire. The core writes
//!   its records, then stores their count. The runtime loads the count before
//!   it reads the records, and stores 0 once it is done with them. The core
//!   loads END before it writes records again.
//! - R3. The records, and any memory a record names for the runtime to read
//!   (xpute-gpu's WRITE_BUFFER_AT and WRITE_TEXTURE_AT, an image's bytes),
//!   are the runtime's from the moment the count is stored until the core
//!   loads END as 0. In between, nothing in the core rewrites or frees that
//!   memory, and no copy of it is made. END can mark that moment because the
//!   runtime's last read of the core's memory is when it runs a record, and
//!   the write that runs it copies the bytes before it returns (wgpu's
//!   `write_buffer`, `write_texture`). Code that writes such memory writes it
//!   while the stream is the core's; otherwise it defers only itself, or
//!   writes a second buffer the runtime does not hold. A runtime stores 0
//!   whether or not it ran the records, so a runtime with nothing to run them
//!   on never holds up the core.
//! - R4. A word that a record names for the runtime to answer later has one
//!   writer for good. The ticket of a request is the core's alone: a sequence
//!   the core advances for that request's fence, wrapping, with one request
//!   outstanding on the fence at a time. The fence is the runtime's alone:
//!   once what the request guards is written, and after the result beside it
//!   where the request can fail, the runtime stores there the ticket it
//!   finished, with release. The core takes the request as done when the
//!   fence, loaded with acquire, equals the ticket it asked with; a late
//!   answer to a request that was dropped or superseded never does
//!   (xpute-gpu spec RESULT). A block's status word (the device's STATUS) is
//!   the runtime's alone, stored with release after the block. No word is
//!   written by both sides.
//!
//! A core that finds END not yet 0 writes no records (`is_free`), and the
//! turn leaves what it would have recorded for a later turn.
//!
//! Where the runtime runs the records at once, as a native GPU runtime does
//! on the core's thread, storing END rings it (`on_publish`). A page's
//! runtime runs them when the turn returns.

#[path = "stream.spec.rs"]
pub mod spec;

use core::cell::Cell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::mem::Memory;

pub const WORD: usize = size_of::<u32>();

thread_local! {
    static ON_PUBLISH: Cell<Option<fn()>> = const { Cell::new(None) };
}

/// Sets what `publish` runs once END is stored (`entry::Form::publish`): a
/// device that runs the stream as soon as it is published.
pub fn on_publish(run: Option<fn()>) {
    ON_PUBLISH.with(|c| c.set(run));
}

/// The words of a record: its op and count, then `args`, then `bytes` as
/// their count followed by the bytes in words.
pub const fn record_words(args: usize, bytes: Option<usize>) -> usize {
    2 + args
        + match bytes {
            Some(b) => 1 + b.div_ceil(WORD),
            None => 0,
        }
}

/// A record's size in bytes, the unit a stream's room is counted in.
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
    /// Whether the runtime has stored END back to 0 since the last publish,
    /// as of the last load.
    free: bool,
}

impl Stream {
    pub const END: usize = spec::END as usize;
    pub const HEAD: usize = spec::HEAD as usize;
    /// The stream's own words, before the user's.
    pub const OWN_WORDS: usize = spec::OWN_WORDS as usize;

    /// A stream over `words` words at `at` in `mem`, which must be
    /// word-aligned. A new stream has published nothing, whatever the range
    /// held.
    ///
    /// # Safety
    /// For as long as the stream is used, the range must stay valid, unmoved
    /// and the stream's alone, and the runtime reads it only while END says
    /// so. No runtime may run the stream before `new` returns.
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

    /// The `n` words from word `k`, which must lie past END. They are the
    /// core's while the stream is free.
    #[allow(clippy::mut_from_ref)]
    fn words(&self, k: usize, n: usize) -> &mut [u32] {
        xpute_kit::ensure!(k > Stream::END && k + n <= self.words, ENOSPC, k, n);
        // SAFETY: inside the range `new` was given, as its contract requires,
        // END left out; while the stream is free nothing else reads it.
        unsafe { core::slice::from_raw_parts_mut(self.mem.at::<u32>(self.at + k * WORD), n) }
    }

    /// Where the stream's words begin in its memory, `END` first.
    pub fn at(&self) -> usize {
        self.at
    }

    pub fn left(&self) -> usize {
        (self.words - self.head - self.len) * WORD
    }

    /// Whether the runtime has stored END back to 0 since the last publish,
    /// so that the stream is the core's to write (R1, R3).
    pub fn is_free(&mut self) -> bool {
        if !self.free {
            self.free = self.end().load(Ordering::Acquire) == 0;
        }
        self.free
    }

    /// Hands the turn's records to the runtime (R2). A turn that recorded
    /// none leaves END as it is.
    pub fn publish(&mut self) {
        if self.len == 0 {
            return;
        }
        self.end().store(self.len as u32, Ordering::Release);
        self.len = 0;
        self.free = false;
        if let Some(ring) = ON_PUBLISH.with(Cell::get) {
            ring();
        }
    }

    /// Whether records of `bytes` bytes (`record_bytes`, or xpute-gpu
    /// encode's `call::*_bytes`) fit with `leave` bytes left after them.
    pub fn fits(&self, bytes: usize, leave: usize) -> bool {
        self.left() >= bytes + leave
    }

    /// Opens a record of `n` words for its writer to fill. A record past the
    /// range, or one opened while the runtime holds the stream (EBUSY), is a
    /// bug, reported at the caller, which names the stream and the record.
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

    /// Begins a transaction: the records from here to `commit` go as one. A
    /// record that does not fit drops the whole transaction, and every record
    /// reserved after it until `commit` is refused. Outside a transaction, a
    /// record that does not fit is a bug.
    pub fn begin(&mut self) {
        self.mark = Mark::Open(self.len);
    }

    /// Answers false when the transaction did not fit, and the stream is then
    /// as it was at `begin`.
    pub fn commit(&mut self) -> bool {
        let mark = core::mem::replace(&mut self.mark, Mark::Off);
        if let Mark::Spilled(at) = mark {
            self.len = at;
        }
        !matches!(mark, Mark::Spilled(_))
    }

    /// A record of `n` words for its writer to fill, or none when it no
    /// longer fits in an open transaction, which is then dropped.
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

/// Fills a record's words past its op and count: `args`, `tail`, then
/// `bytes` as their count followed by the bytes in words.
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
