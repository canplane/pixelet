// xpute-link/mem.test.rs

//! The guest's memory for this process's tests, as a native guest sets its
//! own (`set`): one region, set at its first use.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Once;

use super::*;

/// Takes `len` bytes of the guest's memory, given to no other test, and
/// answers where they lie.
pub(crate) fn alloc(len: u32) -> u32 {
    const SZ: u32 = 1 << 20;
    static SET: Once = Once::new();
    static BRK: AtomicU32 = AtomicU32::new(0);
    SET.call_once(|| {
        let base = Vec::leak(vec![0u64; SZ as usize / 8]).as_mut_ptr().cast();
        set(Region { base, len: SZ });
    });
    let at = BRK.fetch_add(len.next_multiple_of(8), Ordering::Relaxed);
    assert!(at + len <= SZ, "the tests' memory is spent");
    at
}
