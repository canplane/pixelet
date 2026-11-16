// pxl-gpu-guest/admit.rs

//! Admission says whether the GPU's host can take more now without blocking,
//! by one rule for each thing it holds. A guest admits the writes that may
//! wait only as far as the host's belt has free chunks for them
//! (`free_chunk_cnt`, stream.rs `pack`), hands the host its records (`flush`)
//! while the device is ready, and draws a frame (`frame`) only where it may
//! flush and fewer of its frames than its frame latency are unfinished on the
//! GPU. Every host writes the frame
//! latency its program chose, its unfinished frames and its free chunks into
//! the device's block (spec `DEVICE`), and the rules are applied there.

use std::cell::Cell;

use pxl_gpu_link::spec::device;
use pxl_gpu_link::spec::status;

use crate::encode::load_acquire;

thread_local! {
    /// The first word of the device's block the guest last lent
    /// (spec/gpu/wit/gpu.wit `device.lost`).
    static BLOCK: Cell<Option<*const u32>> = const { Cell::new(None) };
}

/// Keeps the block the guest lends the host, for `flush`, `frame` and
/// `block` to read. Its words have one writer, the host, so the guest keeps
/// the block in place for as long as it is lent and never writes it.
pub fn ask(block: &[u32]) {
    BLOCK.set(Some(block.as_ptr()));
}

/// The first word of the block `ask` kept, `device::WORDS` long and in place
/// while it is lent, its words to load with acquire (`load_acquire`).
pub fn block() -> Option<*const u32> {
    BLOCK.get()
}

/// Loads the word `k` of the block `ask` kept.
fn word(at: *const u32, k: usize) -> u32 {
    // SAFETY: a word of the block `ask` kept, which stays live and in place;
    // the host stores into it only atomically, and writes the frame latency
    // before `STATUS` and never after.
    unsafe { load_acquire(at.add(k)) }
}

/// How many chunks of the host's belt the block says are free, none before
/// the block is `READY` and none once the device is lost. A writer that may
/// wait asks for no more than they hold (stream.rs `pack`).
pub fn free_chunk_cnt() -> u32 {
    block()
        .filter(|&at| word(at, device::STATUS_AT) == status::READY)
        .map_or(0, |at| word(at, device::FREE_CHUNKS_AT))
}

/// This is the admission of a flush: while the block's `STATUS` is `READY`.
/// It admits none before the guest has asked for the block, and none once
/// the device is lost.
pub fn flush() -> bool {
    block().is_some_and(|at| word(at, device::STATUS_AT) == status::READY)
}

/// This is the admission of a frame: where a flush is admitted, while the
/// block's unfinished frames are fewer than its frame latency.
pub fn frame() -> bool {
    flush()
        && block().is_some_and(|at| word(at, device::UNFINISHED_AT) < word(at, device::LATENCY_AT))
}
