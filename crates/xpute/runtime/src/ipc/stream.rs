// xpute-runtime/ipc/stream.rs

//! A turn's calls from guest to host as records, run whole by the host after
//! the turn returns, so a record from turn N is done when N + 1 rises (a
//! ring gives no such guarantee).
//!
//!   0      END      words of records past the head, set at the turn's end
//!   1…     the user's
//!   head…  op, n, n words; …   bytes go as their count, then padded to a word

pub const END: usize = 0;

pub struct Stream {
    words: &'static mut [u32],
    head: usize,
    len: usize,
}

impl Stream {
    pub fn new(words: &'static mut [u32], head: usize) -> Stream {
        xpute_core::ensure!(head > END && head < words.len(), EINVAL);
        Stream { words, head, len: 0 }
    }

    pub fn head(&self) -> &[u32] {
        &self.words[..self.head]
    }

    pub fn left(&self) -> usize {
        (self.words.len() - self.head - self.len) * 4
    }

    pub fn publish(&mut self) {
        self.words[END] = self.len as u32;
        self.len = 0;
    }

    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.head + self.len;
        xpute_core::ensure!(at + 2 + n <= self.words.len(), ENOSPC, self.words.len() * 4);
        self.len += 2 + n;
        let w = &mut self.words[at..at + 2 + n];
        w[0] = op;
        w[1] = n as u32;
        &mut w[2..]
    }

    pub fn record(&mut self, op: u32, args: &[u32], tail: &[u32], bytes: Option<&[u8]>) {
        let byte_words = bytes.map_or(0, |b| 1 + b.len().div_ceil(4));
        let w = self.open(op, args.len() + tail.len() + byte_words);
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
