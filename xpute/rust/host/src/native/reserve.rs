// xpute-host/native/reserve.rs

//! The core memory, reserved once from the system and never given back:
//! mapped as a WebAssembly memory is, so its pages are zero until written and
//! cost nothing until touched, where the system's `alloc_zeroed` would write
//! every byte and make the whole memory resident at boot.

use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use xpute_guest::mem::section::PAGE_BYTES;

/// The memory's first byte, 0 until it is reserved, and its bytes.
static AT: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

/// Maps `bytes` from the system: once, by `boot`, which is once a process.
/// A mapping lies on the system's page, at least the guest's, which is all
/// the heap aligns to. Its address and its bytes.
pub fn reserve(bytes: usize) -> (usize, usize) {
    // SAFETY: an anonymous private mapping, which touches nothing that exists.
    let p = unsafe { libc::mmap(core::ptr::null_mut(), bytes, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0) };
    assert!(p != libc::MAP_FAILED, "the system would not map a guest's {bytes} bytes");
    let at = p.cast::<u8>().expose_provenance();
    assert!(at.is_multiple_of(PAGE_BYTES as usize), "the system mapped the memory off its page");
    BYTES.store(bytes, Ordering::Relaxed);
    AT.store(at, Ordering::Release);
    (at, bytes)
}

/// The memory's address and bytes, (0, 0) before it is reserved.
pub fn bounds() -> (usize, usize) {
    let at = AT.load(Ordering::Acquire);
    (at, BYTES.load(Ordering::Relaxed))
}

/// Whether `p` lies in the memory; on any thread, and false before it is
/// reserved.
pub fn holds(p: *const u8) -> bool {
    let (at, bytes) = bounds();
    at != 0 && (at..at + bytes).contains(&p.addr())
}

/// The memory as a pointer and its bytes, once reserved.
pub fn reserved() -> (NonNull<u8>, usize) {
    let (at, bytes) = bounds();
    let base = NonNull::new(core::ptr::with_exposed_provenance_mut::<u8>(at)).expect("the memory is reserved at boot");
    (base, bytes)
}
