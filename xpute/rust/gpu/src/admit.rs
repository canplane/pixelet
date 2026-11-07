// xpute-gpu/admit.rs

//! Whether the GPU's runtime takes a frame now without blocking: a core draws a
//! frame only while fewer of its frames than its frame latency are unfinished
//! on the GPU (`admits`), the one rule every runtime holds a frame to. A runtime in
//! the core's own process applies it to its own count
//! (`runtime::native::admit_frame` over `GpuRuntime::admit_frame`); a page's runtime tells the
//! core its count in the device's block (spec DEVICE), and `page` applies it
//! there. Either reaches the core with its boot (xpute-core
//! `BootInfo::admit`).

use core::sync::atomic::{AtomicU32, Ordering};

use xpute_core::global::Global;

use crate::spec::STATUS;

pub type Admit = fn() -> bool;

/// Every frame: where a core's form hands no admission.
pub fn every() -> bool {
    true
}

/// A frame while fewer than `latency` drawn are unfinished.
pub fn admits(unfinished: u32, latency: u32) -> bool {
    unfinished < latency
}

/// The device's block the core last asked for (`call::device`): its first
/// word and its length.
static BLOCK: Global<Option<(*mut u32, usize)>> = Global::new();

/// The block `call::device` names to the runtime, kept for `page`. A block's
/// words are the runtime's to write for good (xpute-core ipc/stream.rs R4), so
/// it stays where it is while it is read.
pub(crate) fn ask(block: &mut [u32]) {
    BLOCK.set(Some((block.as_mut_ptr(), block.len())));
}

/// The page's admission (`BootInfo::admit` of a WebAssembly core given the
/// GPU, xpute-core `core!`), `LATENCY` its program's frame latency
/// (`Numbers`): a frame while the block's device is ready and its frames
/// unfinished admit one; none before the block is asked for or once the
/// device is lost.
pub fn page<const LATENCY: u32>() -> bool {
    BLOCK.get(|| None).is_some_and(|(at, len)| {
        // SAFETY: the block `ask` kept, which stays live and in place; its
        // words the runtime writes are only ever loaded atomically.
        let word = |k: usize| unsafe { AtomicU32::from_ptr(at.add(k)) }.load(Ordering::Acquire);
        word(len - 1) == STATUS::READY && admits(word(len - 2), LATENCY)
    })
}

#[cfg(test)]
#[path = "admit.test.rs"]
mod test;
