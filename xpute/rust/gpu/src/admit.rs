// xpute-gpu/admit.rs

//! Admission says whether the GPU's runtime can take another frame now without
//! blocking. Every runtime holds a frame to one rule (`admits`): a core draws a
//! frame only while fewer of its frames than its frame latency are unfinished
//! on the GPU. A runtime in the core's own process applies the rule to its own
//! count (`runtime::native::admit_frame`). A page's runtime writes its count
//! into the device's block (spec DEVICE), and `page` applies the rule there.
//! A program asks through what `core!` defines (`admit_frame`).

use core::sync::atomic::{AtomicU32, Ordering};

use xpute_core::global::Global;

use crate::device::{STATUS_AT, UNFINISHED_AT};
use crate::spec::STATUS;

pub fn admits(unfinished: u32, latency: u32) -> bool {
    unfinished < latency
}

/// This is the first word of the device's block that the core last asked for
/// (`call::device`).
static BLOCK: Global<Option<*mut u32>> = Global::new();

/// Keeps the block that `call::device` names to the runtime, for `page` to
/// read. The runtime writes the block's words for good (xpute-core
/// io/stream.rs R4), so the core keeps the block in place and never writes
/// it.
pub(crate) fn ask(block: &mut [u32]) {
    BLOCK.set(Some(block.as_mut_ptr()));
}

/// This is the admission of a WebAssembly core that has the GPU (this crate's
/// `core!`), where `LATENCY` is the program's frame latency (`CoreConfig`). It
/// admits a frame while the block's STATUS is READY and `admits` holds for the
/// block's unfinished frames. It admits none before the core has asked for the
/// block, and none once the device is lost.
pub fn page<const LATENCY: u32>() -> bool {
    BLOCK.get(|| None).is_some_and(|at| {
        // SAFETY: the block `ask` kept, which stays live and in place; its
        // words the runtime writes are only ever loaded atomically.
        let word = |k: usize| unsafe { AtomicU32::from_ptr(at.add(k)) }.load(Ordering::Acquire);
        word(STATUS_AT) == STATUS::READY && admits(word(UNFINISHED_AT), LATENCY)
    })
}

#[cfg(test)]
#[path = "admit.test.rs"]
mod test;
