// xpute-link/mem.test.rs

//! The guest's memory for this process's tests, which are its runtime
//! (`region`): one region, reserved at its first use, all of it the tests'
//! share.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

const SZ: u32 = 1 << 20;

/// The tests' region, as a runtime answers it.
#[unsafe(no_mangle)]
extern "C" fn xpute_mem(len: *mut u32, share_sz: *mut u32) -> *mut u8 {
    static BASE: OnceLock<usize> = OnceLock::new();
    let base = *BASE.get_or_init(|| {
        Vec::leak(vec![0u64; SZ as usize / 8])
            .as_mut_ptr()
            .expose_provenance()
    });
    // SAFETY: the caller's words.
    unsafe { (*len, *share_sz) = (SZ, SZ) };
    core::ptr::with_exposed_provenance_mut(base)
}

/// Takes `len` bytes of the guest's memory, given to no other test, and
/// answers where they lie.
pub(crate) fn alloc(len: u32) -> u32 {
    static BRK: AtomicU32 = AtomicU32::new(0);
    let at = BRK.fetch_add(len.next_multiple_of(8), Ordering::Relaxed);
    assert!(at + len <= SZ, "the tests' memory is spent");
    at
}
