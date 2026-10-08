// xpute-guest/ipc/stream.rs

//! A turn's calls from guest to host as records, run whole by the host after
//! the turn returns, so a record from turn N is done when N + 1 rises (a
//! ring gives no such guarantee).
//!
//!   0      END      words of records past the head, set at the turn's end
//!   1…     the user's
//!   head…  op, n, n words; …   bytes go as their count, then padded to a word

pub const END: usize = 0;

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
    words: &'static mut [u32],
    head: usize,
    len: usize,
    /// Where an open transaction began, and whether a record past the range
    /// has dropped it.
    mark: Option<usize>,
    spilled: bool,
}

impl Stream {
    pub fn new(words: &'static mut [u32], head: usize) -> Stream {
        xpute_kit::ensure!(head > END && head < words.len(), EINVAL);
        Stream {
            words,
            head,
            len: 0,
            mark: None,
            spilled: false,
        }
    }

    pub fn head(&self) -> &[u32] {
        &self.words[..self.head]
    }

    pub fn left(&self) -> usize {
        (self.words.len() - self.head - self.len) * WORD
    }

    pub fn publish(&mut self) {
        self.words[END] = self.len as u32;
        self.len = 0;
    }

    /// Whether a record of `args` words and `bytes` would fit whole with
    /// `reserve` bytes still left after it.
    pub fn fits(&self, args: usize, bytes: Option<usize>, reserve: usize) -> bool {
        self.left() >= record_bytes(args, bytes) + reserve
    }

    /// A bug past the range reports its caller, which names the stream and the record.
    #[track_caller]
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.head + self.len;
        xpute_kit::ensure!(at + 2 + n <= self.words.len(), ENOSPC, self.words.len() * 4);
        self.len += 2 + n;
        let w = &mut self.words[at..at + 2 + n];
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

    #[track_caller]
    pub fn record(&mut self, op: u32, args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
        if self.mark.is_some() && (self.spilled || !self.fits(args.len() + tail.len(), bytes.map(<[u8]>::len), 0)) {
            self.spilled = true;
            return;
        }
        let n = record_words(args.len() + tail.len(), bytes.map(<[u8]>::len)) - 2;
        let w = self.open(op, n);
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
