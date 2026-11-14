// xpute-gpu-guest/admit.rs

//! Admission says whether the GPU's host can take another frame now without
//! blocking. Every host holds a frame to one rule (`admits`): a guest draws a
//! frame only while fewer of its frames than its frame latency are unfinished
//! on the GPU. Every host writes its count into the device's block (spec
//! DEVICE), and `frame` applies the rule there.

use std::cell::Cell;

use xpute_gpu_link::admit::admits;
use xpute_gpu_link::spec::device;
use xpute_gpu_link::spec::status;

use crate::encode::load_acquire;

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

/// The first word of the block `ask` kept, `device::WORDS` long and in place
/// while it is lent, its words to load with acquire (`load_acquire`).
pub fn block() -> Option<*const u32> {
    BLOCK.get()
}

/// This is the admission of a guest that has the GPU, where `LATENCY` is the
/// program's frame latency (`GpuConfig`). It admits a
/// frame while the block's STATUS is READY and `admits` holds for the block's
/// unfinished frames. It admits none before the guest has asked for the block,
/// and none once the device is lost.
pub fn frame<const LATENCY: u32>() -> bool {
    block().is_some_and(|at| {
        // SAFETY: a word of the block `ask` kept, which stays live and in
        // place; the host stores into it only atomically.
        let word = |k: usize| unsafe { load_acquire(at.add(k)) };
        word(device::STATUS_AT) == status::READY && admits(word(device::UNFINISHED_AT), LATENCY)
    })
}
