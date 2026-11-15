// pxl-gpu-guest/stream.rs

//! A stream carries the guest's GPU calls to the host as records: op, n, then
//! n words; bytes go as their count, then the bytes padded to a word
//! (spec/gpu/api.json5). The guest writes the records into a buffer it
//! owns and flushes them by lending the buffer to the host's `gpu.run`
//! (spec/gpu/wit/gpu.wit), which answers once they have run. From the flush to
//! the answer the records, and any memory a record names for the host to
//! read (the bytes a buffer's or an image's upload takes), are the host's:
//! nothing in the guest rewrites or frees that memory, and no copy of it is
//! made. The host's last read of the guest's memory is when it runs a
//! record, and the write that runs it copies the bytes before it returns, as
//! WebGPU's `writeBuffer` and `writeTexture` do. A host answers whether or
//! not it ran the records, so one with nothing to run them on never holds up
//! the guest.
//!
//! A word that a record names for the host to answer later has one writer
//! for good. The ticket of a request is the guest's alone: a sequence the
//! guest advances for that request's fence, wrapping, with one request
//! outstanding on the fence at a time. The fence is the host's alone: once
//! what the request guards is written, and after the result beside it where
//! the request can fail, the host stores there the ticket it finished, with
//! release. The guest takes the request as done when the fence, loaded with
//! acquire, equals the ticket it asked with; a late answer to a request that
//! was dropped or superseded never does. No word is written by both sides.
//!
//! While a run is out the stream is not free (`is_free`), and the guest
//! writes no records: what it would have recorded waits for a later frame.

use core::pin::Pin;
use core::task::{Context, Poll, Waker};

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

/// A mark says whether a transaction is open, where it began, and whether a
/// record that did not fit has dropped it.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Off,
    Open(usize),
    Spilled(usize),
}

/// A run of the stream's records, out to the host's `gpu.run`.
type Run = Pin<Box<dyn Future<Output = (Vec<u32>, Result<(), Errno>)>>>;

/// Who holds the stream's buffer: the guest, with the records recorded since
/// the last flush, or a run, which gives it back with its answer.
enum Buf {
    Free(Vec<u32>),
    Out(Run),
}

pub struct Stream {
    buf: Buf,
    cap: usize,
    mark: Mark,
}

impl Stream {
    /// A stream of `cap` words.
    pub fn new(cap: usize) -> Stream {
        Stream {
            buf: Buf::Free(Vec::with_capacity(cap)),
            cap,
            mark: Mark::Off,
        }
    }

    /// The words recorded since the last flush.
    fn len(&self) -> usize {
        match &self.buf {
            Buf::Free(words) => words.len(),
            Buf::Out(_) => 0,
        }
    }

    pub fn left(&self) -> usize {
        (self.cap - self.len()) * WORD
    }

    /// Whether no run holds the stream, so that it is the guest's to write.
    pub fn is_free(&mut self) -> bool {
        if let Buf::Out(run) = &mut self.buf {
            let mut cx = Context::from_waker(Waker::noop());
            if let Poll::Ready((mut words, _)) = run.as_mut().poll(&mut cx) {
                words.clear();
                self.buf = Buf::Free(words);
            }
        }
        matches!(self.buf, Buf::Free(_))
    }

    /// Hands the records to the host's `gpu` by lending them to its `run`
    /// (spec/gpu/wit/gpu.wit), called by its named handle. A stream that
    /// recorded none, or that a run still holds, sends nothing.
    pub fn flush(&mut self) {
        if self.len() == 0 || !self.is_free() {
            return;
        }
        if let Buf::Free(words) = &mut self.buf {
            let words = core::mem::take(words);
            self.buf = Buf::Out(Box::pin(gpu::run(words)));
        }
    }

    /// `fits` says whether records of `bytes` bytes fit with
    /// `leave` bytes left after them.
    pub fn fits(&self, bytes: usize, leave: usize) -> bool {
        self.left() >= bytes + leave
    }

    /// Opens a record of `n` words for its writer to fill. A record past the
    /// cap, or one opened while a run holds the stream (EBUSY), is a bug,
    /// reported at the caller, which names the record.
    #[track_caller]
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.len();
        xpute_kit::ensure!(at + 2 + n <= self.cap, ENOSPC, self.cap * WORD);
        xpute_kit::ensure!(self.is_free(), EBUSY, op);
        let Buf::Free(words) = &mut self.buf else {
            xpute_kit::bug!(EBUSY, op);
        };
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
        if let (Mark::Spilled(at), Buf::Free(words)) = (mark, &mut self.buf) {
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
            core::ptr::copy_nonoverlapping(b.as_ptr(), dst.as_mut_ptr().cast::<u8>(), b.len());
        };
    }
}

#[cfg(test)]
#[path = "program.test.rs"]
mod program;
