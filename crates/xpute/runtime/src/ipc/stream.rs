// xpute-runtime/ipc/stream.rs

//! A turn's calls from a guest to its host, as records in one range of the
//! memory both sides see: the guest writes them during a turn, the host runs
//! them once the turn has returned and before the next begins, and nothing
//! else joins the two. A call that would have been an import is a record.
//!
//! What a ring is not for. A ring's message waits until the host reads it, so
//! a guest cannot know when that is; a stream is run whole on the falling
//! edge, so a record written in turn N has been run by the time turn N + 1
//! rises — which is what lets a record name memory the host writes into and
//! the guest read it on the next turn without being told.
//!
//! The range opens on `head` words, of which word 0 is END: how many words of
//! records the turn wrote, set when the turn ends. The rest of the head is the
//! user's. Past it, a record is its op, the count of the words that follow,
//! and those words; bytes go as their count, then the bytes padded to a word.
//!
//!   0      END      words of records past the head
//!   1…     the user's
//!   head…  op, n, n words; op, n, n words; …
//!
//! The ops are the user's: two streams over two ranges agree on this layout
//! and nothing else.

/// The head word holding how many words of records the turn wrote.
pub const END: usize = 0;

pub struct Stream {
    words: &'static mut [u32],
    head: usize,
    len: usize,
}

impl Stream {
    /// A stream over `words`, its first `head` words the head.
    pub fn new(words: &'static mut [u32], head: usize) -> Stream {
        xpute_core::ensure!(head > END && head < words.len(), EINVAL);
        Stream { words, head, len: 0 }
    }

    /// The head, END among it.
    pub fn head(&self) -> &[u32] {
        &self.words[..self.head]
    }

    /// Bytes left for records this turn.
    pub fn left(&self) -> usize {
        (self.words.len() - self.head - self.len) * 4
    }

    /// Ends the turn's records: END says how many there are, and the next
    /// turn writes from the start.
    pub fn publish(&mut self) {
        self.words[END] = self.len as u32;
        self.len = 0;
    }

    /// Room for a record of `n` words after its op and count: the record's
    /// words, to be filled.
    pub fn open(&mut self, op: u32, n: usize) -> &mut [u32] {
        let at = self.head + self.len;
        xpute_core::ensure!(at + 2 + n <= self.words.len(), ENOSPC, self.words.len() * 4);
        self.len += 2 + n;
        let w = &mut self.words[at..at + 2 + n];
        w[0] = op;
        w[1] = n as u32;
        &mut w[2..]
    }

    /// A record of `op`: `args`, then `tail`, then `bytes` if any.
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
            unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), dst.as_mut_ptr() as *mut u8, b.len()) };
        }
    }
}
