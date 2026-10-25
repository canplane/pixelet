// xpute-gpu/host/word.rs

use crate::spec::NONE;

/// A record's words, read in order.
pub(super) struct Words<'w> {
    w: &'w [u32],
    at: usize,
}

impl<'w> Words<'w> {
    pub(super) fn new(w: &'w [u32]) -> Words<'w> {
        Words { w, at: 0 }
    }
    pub(super) fn u(&mut self) -> u32 {
        let v = self.w[self.at];
        self.at += 1;
        v
    }
    pub(super) fn i(&mut self) -> i32 {
        self.u() as i32
    }
    pub(super) fn f32(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    pub(super) fn flag(&mut self) -> bool {
        self.u() != 0
    }
    /// A count, then that many words.
    pub(super) fn list(&mut self) -> &'w [u32] {
        let n = self.u() as usize;
        let words = &self.w[self.at..self.at + n];
        self.at += n;
        words
    }
    pub(super) fn opt(&mut self) -> Option<u32> {
        Some(self.u()).filter(|&v| v != NONE)
    }
    /// A count, then the bytes, padded to words.
    pub(super) fn bytes(&mut self) -> &'w [u8] {
        let n = self.u() as usize;
        let words = &self.w[self.at..self.at + n.div_ceil(4)];
        self.at += n.div_ceil(4);
        // SAFETY: the words as their bytes, which are the guest's in order on
        // a little-endian host, as every host is; at least `n` of them.
        let all: &'w [u8] = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), 4 * words.len()) };
        &all[..n]
    }
    pub(super) fn text(&mut self) -> String {
        String::from_utf8_lossy(self.bytes()).into_owned()
    }
}
