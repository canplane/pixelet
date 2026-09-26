// xpute-runtime/mem/base.rs

//! Where the memory begins, and the one way to an address in it.
//!
//! Every address a module names — a heap block, a range, a ring, what a
//! record hands its host — is an offset from this base and never a pointer,
//! so a number means the same on every host and fits the `u32` the records
//! carry however wide the machine's pointers are. On wasm32 the memory is the
//! linear memory, addressed from 0, and the base is 0. A native build is
//! given one region by its bracket and names everything from that region's
//! start. A native test sets none: its base is 0 and an offset is the
//! address.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

#[cfg(not(target_arch = "wasm32"))]
use crate::global::Global;

/// The memory: where it starts, as an address, and how many bytes it spans.
#[cfg(not(target_arch = "wasm32"))]
static MEMORY: Global<(usize, usize)> = Global::new();

/// Sets the memory to the region the bracket gave the module, `bytes` from
/// `addr`: once, before anything is allocated.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_base(addr: usize, bytes: usize) {
    MEMORY.set((addr, bytes));
}

#[cfg(not(target_arch = "wasm32"))]
fn memory() -> (usize, usize) {
    *MEMORY.get(|| (0, usize::MAX))
}

/// The linear memory, from its first byte: its span is whatever the module
/// has grown to, and nothing a module names lies past it.
#[cfg(target_arch = "wasm32")]
const fn memory() -> (usize, usize) {
    (0, usize::MAX)
}

/// The base, as an address.
pub fn base() -> usize {
    memory().0
}

/// A pointer `off` bytes past the base.
///
/// Made from the address and not by offsetting a pointer. On wasm the base is
/// a number, not a pointer into an allocation, and `ptr::add` from one is
/// undefined: a release build took the license and dropped the stores that
/// followed — a ring's capacity word went missing from an otherwise correct
/// header. What an offset lies in was exposed when it was named (`off_of`),
/// so the address recovers it.
pub fn ptr_at<T>(off: usize) -> *mut T {
    core::ptr::with_exposed_provenance_mut(base() + off)
}

/// Where `p` lies, as an offset from the base. A pointer outside the memory
/// is a bug: what the module names has to be in it, and on a native build a
/// static or the binary's read-only data is not — it is where the linker put
/// it, not in the region the bracket gave.
#[track_caller]
pub fn off_of<T>(p: *const T) -> usize {
    let (base, bytes) = memory();
    p.expose_provenance().checked_sub(base).filter(|&off| off < bytes).or_bug(Errno::EFAULT)
}

/// Where `s` lies, as `off_of` says, for a reader told its length too. An
/// empty slice owns no bytes and its pointer is only its alignment — inside
/// wasm's memory by chance, outside a native build's region — so it names 0,
/// where nothing is read. A slice the host writes into is passed as
/// `&mut`, so what is exposed may be written through.
#[track_caller]
pub fn off_of_slice<T>(s: *const [T]) -> usize {
    if s.is_empty() {
        0
    } else {
        off_of(s.cast::<T>())
    }
}
