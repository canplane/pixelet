// xpute-core/io/head.rs

//! `Head` is the header of the memory's front (spec/io/head.json; the front is
//! described in manifest.rs). A runtime writes these words in the page past the
//! stack before the first turn, and they say how much a frame's packet takes
//! and where the stream it laid out lies. A core checks the header against its
//! manifest's numbers (io/door.rs `boot`). A core that lays out its own front, as
//! the conformance core does, writes the header itself (`write`).

#[path = "head.spec.rs"]
pub mod spec;

use crate::mem::Memory;
use spec::word;

/// The header's words after MAGIC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub pkt_bytes: u32,
    /// 0 without a stream.
    pub stream_at: u32,
}

impl Head {
    pub const fn words(&self) -> [u32; spec::WORDS as usize] {
        let mut w = [0; spec::WORDS as usize];
        w[word::MAGIC as usize] = spec::MAGIC;
        w[word::PKT_BYTES as usize] = self.pkt_bytes;
        w[word::STREAM_AT as usize] = self.stream_at;
        w
    }
}

/// # Safety
/// The header's words at `at` must be valid, and nothing else may hold them
/// while they are written.
pub unsafe fn write(mem: Memory, at: usize, head: &Head) {
    for (i, w) in head.words().into_iter().enumerate() {
        mem.set_word(at + 4 * i, w);
    }
}

/// Finds the first word at `at` that differs from `head`'s, and answers
/// where it is in the memory and what it holds, or none when all agree.
///
/// # Safety
/// The header's words at `at` must be valid to read.
pub unsafe fn differs(mem: Memory, at: usize, head: &Head) -> Option<(u32, u32)> {
    head.words().into_iter().enumerate().find_map(|(i, w)| {
        let off = at + 4 * i;
        let found = mem.word(off);
        (found != w).then_some((off as u32, found))
    })
}
