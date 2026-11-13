// xpute-gpu-guest/admit.rs

//! Admission says whether the GPU's host can take another frame now without
//! blocking. Every host holds a frame to one rule (`admits`): a guest draws a
//! frame only while fewer of its frames than its frame latency are unfinished
//! on the GPU. Every host writes its count into the device's block (spec
//! DEVICE), and `frame` applies the rule there.

use core::sync::atomic::{AtomicU32, Ordering};
use std::cell::Cell;

use xpute_gpu_link::admit::admits;
use xpute_gpu_link::spec::DEVICE;
use xpute_gpu_link::spec::STATUS;

thread_local! {
    /// The first word of the device's block the guest last lent
    /// (gpu/wit/gpu.wit `device.lost`).
    static BLOCK: Cell<Option<*const u32>> = const { Cell::new(None) };
}

/// Keeps the block the guest lends the host, for `frame` and `block` to
/// read. Its words have one writer, the host, so the guest keeps the block
/// in place for as long as it is lent and never writes it.
pub fn ask(block: &[u32]) {
    BLOCK.set(Some(block.as_ptr()));
}

/// The block `ask` kept, its words to load with acquire.
pub fn block() -> Option<&'static [u32]> {
    // SAFETY: the block `ask` kept, in place while it is lent.
    BLOCK.get().map(|at| unsafe { core::slice::from_raw_parts(at, DEVICE::WORDS) })
}

/// This is the admission of a guest that has the GPU, where `LATENCY` is the
/// program's frame latency (`GpuConfig`). It admits a
/// frame while the block's STATUS is READY and `admits` holds for the block's
/// unfinished frames. It admits none before the guest has asked for the block,
/// and none once the device is lost.
pub fn frame<const LATENCY: u32>() -> bool {
    BLOCK.get().is_some_and(|at| {
        // SAFETY: the block `ask` kept, which stays live and in place; its
        // words the host writes are only ever loaded atomically.
        let word = |k: usize| unsafe { AtomicU32::from_ptr(at.add(k).cast_mut()) }.load(Ordering::Acquire);
        word(DEVICE::STATUS_AT) == STATUS::READY && admits(word(DEVICE::UNFINISHED_AT), LATENCY)
    })
}
