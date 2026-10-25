// xpute-gpu/admit.rs

//! Whether the GPU's host takes a frame now without blocking: a guest draws
//! a frame only while it is admitted, and its host counts one admitted until
//! the GPU has done it. A host in the guest's own process answers on the
//! guest's thread (`set`), as a native one does from its frame latency
//! (`host` `GpuHost::admit`). One that is not, a page's, leaves every frame
//! admitted: the browser paces a page's frames to what the GPU finishes, and
//! taking a canvas's texture never blocks.

use core::cell::Cell;

thread_local! {
    static ADMIT: Cell<Option<fn() -> bool>> = const { Cell::new(None) };
}

/// The host's answer, from here on, on this thread.
pub fn set(admit: fn() -> bool) {
    ADMIT.with(|a| a.set(Some(admit)));
}

/// Whether a frame drawn now is taken without blocking: the host's answer,
/// and yes where no host has answered.
pub fn admitted() -> bool {
    ADMIT.with(Cell::get).is_none_or(|f| f())
}
