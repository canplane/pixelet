// xpute-core/ipc/head.rs

//! `Head` is the header of the memory's front (spec/ipc/head.json; the front is
//! described in machine.rs). A runtime writes these words in the page past the
//! stack before the first turn, and they say where the rings and the GPU
//! stream it laid out lie. A core of a machine checks the header against the
//! machine's numbers (machine.rs `boot`). A core that lays out its own front,
//! as the conformance core does, writes the header itself (`write`).

#[path = "head.spec.rs"]
pub mod spec;

use crate::mem::Memory;
use spec::word;

/// The header's words after MAGIC.
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
    pub const fn words(&self) -> [u32; spec::WORDS as usize] {
        let mut w = [0; spec::WORDS as usize];
        w[word::MAGIC as usize] = spec::MAGIC;
        w[word::SQ_CAP as usize] = self.sq_cap;
        w[word::CQ_CAP as usize] = self.cq_cap;
        w[word::SLOT_BYTES as usize] = self.slot_bytes;
        w[word::SQ_AT as usize] = self.sq_at;
        w[word::CQ_AT as usize] = self.cq_at;
        w[word::GPU_AT as usize] = self.gpu_at;
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
