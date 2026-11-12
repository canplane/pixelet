// xpute-core/mem.rs

//! The core's memory, which every offset on the link names: offset `at` is
//! the byte `at` past the memory's base. A module's memory begins at 0.

const BASE: usize = 0;

/// The `len` bytes at `at`.
///
/// # Safety
/// They lie in the core's memory, and nothing else writes them while the
/// slice lives.
pub unsafe fn bytes<'a>(at: u32, len: u32) -> &'a mut [u8] {
    let p = core::ptr::with_exposed_provenance_mut::<u8>(BASE + at as usize);
    // SAFETY: the caller's.
    unsafe { core::slice::from_raw_parts_mut(p, len as usize) }
}

/// The offset of `p` in the core's memory.
pub fn off(p: *const u8) -> u32 {
    (p.expose_provenance() - BASE) as u32
}
