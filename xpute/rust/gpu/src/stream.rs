// xpute-gpu/stream.rs

//! A stream carries the core's GPU calls to the runtime as records: op, n,
//! then n words; bytes go as their count, then the bytes padded to a word
//! (spec/gpu api.json). The core writes the records into a buffer it owns and
//! flushes them by lending the buffer to the runtime's `gpu.run`
//! (wit/gpu/gpu.wit), which answers once they have run. From the flush to
//! the answer the records, and any memory a record names for the runtime to
//! read (the bytes a buffer's or an image's upload takes), are the runtime's:
//! nothing in the core rewrites or frees that memory, and no copy of it is
//! made. The runtime's last read of the core's memory is when it runs a
//! record, and the write that runs it copies the bytes before it returns, as
//! WebGPU's `writeBuffer` and `writeTexture` do. A runtime answers whether or
//! not it ran the records, so one with nothing to run them on never holds up
//! the core.
//!
//! A word that a record names for the runtime to answer later has one writer
//! for good. The ticket of a request is the core's alone: a sequence the core
//! advances for that request's fence, wrapping, with one request outstanding
//! on the fence at a time. The fence is the runtime's alone: once what the
//! request guards is written, and after the result beside it where the
//! request can fail, the runtime stores there the ticket it finished, with
//! release. The core takes the request as done when the fence, loaded with
//! acquire, equals the ticket it asked with; a late answer to a request that
//! was dropped or superseded never does. No word is written by both sides.
//!
//! While a run is out the stream is not free (`is_free`), and the core writes
//! no records: what it would have recorded waits for a later frame.

use std::cell::Cell;
use std::rc::Rc;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::gpu;

pub const WORD: usize = size_of::<u32>();

/// A record's words are its op and count, then `args`, then `bytes` as their
/// count followed by the bytes in words.
pub const fn rec_words(args: usize, bytes: Option<usize>) -> usize {
    2 + args
        + match bytes {
            Some(b) => 1 + b.div_ceil(WORD),
            None => 0,
        }
}

/// `rec_bytes` is a record's size in bytes, the unit a stream's room is
/// counted in.
pub const fn rec_bytes(args: usize, bytes: Option<usize>) -> usize {
    rec_words(args, bytes) * WORD
}

/// A mark says whether a transaction is open, where it began, and whether a
/// record that did not fit has dropped it.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Off,
    Open(usize),
    Spilled(usize),
}

pub struct Stream {
    /// The records recorded since the last flush, in a buffer of the
    /// stream's room; none while a run holds them.
    words: Option<Vec<u32>>,
    /// Where a run's answer gives the buffer back.
    back: Rc<Cell<Option<Vec<u32>>>>,
    room: usize,
    mark: Mark,
}

impl Stream {
    /// A stream of `room` words.
    pub fn new(room: usize) -> Stream {
        Stream {
            words: Some(Vec::with_capacity(room)),
            back: Rc::new(Cell::new(None)),
            room,
            mark: Mark::Off,
        }
    }

    /// The words recorded since the last flush.
    fn len(&self) -> usize {
        self.words.as_ref().map_or(0, Vec::len)
    }

    pub fn left(&self) -> usize {
        (self.room - self.len()) * WORD
    }

    /// Whether no run holds the stream, so that it is the core's to write.
    pub fn is_free(&mut self) -> bool {
        if self.words.is_none() {
            self.words = self.back.take().map(|mut w| {
                w.clear();
                w
            });
        }
        self.words.is_some()
    }

    /// Hands the records to the runtime's `gpu` by lending them to its `run`
    /// (wit/gpu/gpu.wit), the singleton the core opened at boot. A stream that
    /// recorded none, or that a run still holds, sends nothing.
    pub fn flush(&mut self) {
        if self.len() == 0 || !self.is_free() {
            return;
        }
        let Some(words) = self.words.take() else { return };
        let back = self.back.clone();
        gpu::run_then(words, move |(words, _)| back.set(Some(words)));
    }

    /// `fits` says whether records of `bytes` bytes (`rec_bytes`) fit with
    /// `leave` bytes left after them.
    pub fn fits(&self, bytes: usize, leave: usize) -> bool {
        self.left() >= bytes + leave
    }

    /// Opens a record of `n` words for its writer to fill. A record past the
    /// room, or one opened while a run holds the stream (EBUSY), is a bug,
    /// reported at the caller, which names the record.
    #[track_caller]
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.len();
        xpute_kit::ensure!(at + 2 + n <= self.room, ENOSPC, self.room * WORD);
        xpute_kit::ensure!(self.is_free(), EBUSY, op);
        let words = self.words.as_mut().or_bug(Errno::EBUSY);
        // Within the capacity the stream was made with, so nothing moves.
        words.resize(at + 2 + n, 0);
        let w = &mut words[at..];
        w[0] = op;
        w[1] = n as u32;
        &mut w[2..]
    }

    /// Begins a transaction: the records from here to `commit` go as one. A
    /// record that does not fit drops the whole transaction, and every record
    /// reserved after it until `commit` is refused. Outside a transaction, a
    /// record that does not fit is a bug.
    pub fn begin(&mut self) {
        self.mark = Mark::Open(self.len());
    }

    /// Answers false when the transaction did not fit, and the stream is then
    /// as it was at `begin`.
    pub fn commit(&mut self) -> bool {
        let mark = core::mem::replace(&mut self.mark, Mark::Off);
        if let (Mark::Spilled(at), Some(words)) = (mark, self.words.as_mut()) {
            words.truncate(at);
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
        let n = rec_words(args.len() + tail.len(), bytes.map(<[u8]>::len)) - 2;
        let Some(w) = self.reserve(op, n) else { return };
        fill(w, args, tail, bytes);
    }
}

/// Fills a record's words past its op and count: `args`, `tail`, then `bytes`
/// as their count followed by the bytes in words.
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
