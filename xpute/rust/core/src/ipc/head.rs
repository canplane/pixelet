// xpute-core/ipc/head.rs

//! The front's header (spec/ipc/head.json): the words a runtime writes on the
//! page past the stack before the first turn, naming the rings and the GPU
//! stream it laid. A core of a machine holds the header to the machine's
//! numbers (machine.rs `boot`); a core that lays its own front, as the
//! conformance core does, writes it (`write`).

#[path = "head.spec.rs"]
pub mod spec;

use crate::mem::Memory;
use spec::word;

/// What the header says past MAGIC and VERSION.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub sq_cap: u32,
    pub cq_cap: u32,
    pub slot_bytes: u32,
    pub sq_at: u32,
    pub cq_at: u32,
    /// 0 without the GPU device.
    pub gpu_at: u32,
}

impl Head {
    /// Every word, in the header's order.
    pub const fn words(&self) -> [u32; spec::WORDS as usize] {
        let mut w = [0; spec::WORDS as usize];
        w[word::MAGIC as usize] = spec::MAGIC;
        w[word::VERSION as usize] = spec::VERSION;
        w[word::SQ_CAP as usize] = self.sq_cap;
        w[word::CQ_CAP as usize] = self.cq_cap;
        w[word::SLOT_BYTES as usize] = self.slot_bytes;
        w[word::SQ_AT as usize] = self.sq_at;
        w[word::CQ_AT as usize] = self.cq_at;
        w[word::GPU_AT as usize] = self.gpu_at;
        w
    }
}

/// `head`'s words at `at` in `mem`.
///
/// # Safety
/// The header's words at `at` are valid and nothing else holds them while
/// they are written.
pub unsafe fn write(mem: Memory, at: usize, head: &Head) {
    for (i, w) in head.words().into_iter().enumerate() {
        mem.set_word(at + 4 * i, w);
    }
}

/// The first of the words at `at` that is not `head`'s: where it is in the
/// memory and what it holds. None where all are.
///
/// # Safety
/// The header's words at `at` are valid to read.
pub unsafe fn differs(mem: Memory, at: usize, head: &Head) -> Option<(u32, u32)> {
    head.words().into_iter().enumerate().find_map(|(i, w)| {
        let off = at + 4 * i;
        let found = mem.word(off);
        (found != w).then_some((off as u32, found))
    })
}
