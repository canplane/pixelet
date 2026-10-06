// xpute-runtime/mem/base.rs

//! Every address a module names is an offset from this base, never a
//! pointer, so it means the same on every host and fits a `u32`. The base is
//! 0 on wasm32 and in a native test; a native build sets its region's start.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

#[cfg(not(target_arch = "wasm32"))]
use crate::global::Global;

/// The start address and the span in bytes.
#[cfg(not(target_arch = "wasm32"))]
static MEMORY: Global<(usize, usize)> = Global::new();

/// Once, before anything is allocated.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_base(addr: usize, bytes: usize) {
    MEMORY.set((addr, bytes));
}

#[cfg(not(target_arch = "wasm32"))]
fn memory() -> (usize, usize) {
    *MEMORY.get(|| (0, usize::MAX))
}

#[cfg(target_arch = "wasm32")]
const fn memory() -> (usize, usize) {
    (0, usize::MAX)
}

pub fn base() -> usize {
    memory().0
}

/// Made from the address, not by `ptr::add` on the wasm base, which is UB: a
/// release build once dropped a ring's capacity word for it. `off_of` exposed
/// the provenance this recovers.
pub fn ptr_at<T>(off: usize) -> *mut T {
    core::ptr::with_exposed_provenance_mut(base() + off)
}

/// A pointer outside the memory is a bug; on a native build that includes
/// statics and read-only data.
#[track_caller]
pub fn off_of<T>(p: *const T) -> usize {
    let (base, bytes) = memory();
    p.expose_provenance().checked_sub(base).filter(|&off| off < bytes).or_bug(Errno::EFAULT)
}

/// An empty slice's pointer is only its alignment, possibly outside the
/// region, so it names 0. Pass a slice the host writes into as `&mut`.
#[track_caller]
pub fn off_of_slice<T>(s: *const [T]) -> usize {
    if s.is_empty() {
        0
    } else {
        off_of(s.cast::<T>())
    }
}
