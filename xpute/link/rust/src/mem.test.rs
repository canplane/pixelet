// xpute-link/mem.test.rs

//! The guest's memory for a process's tests: what a test lays its frames,
//! packets and blocks in, taken from the process's own. xpute-link's tests and
//! a plugin's host's take it alike.

/// Takes `len` zeroed bytes, on a word, given to no other test, for the
/// process's life, and answers their address.
pub(crate) fn alloc(len: u32) -> u64 {
    let words: &'static [u64] = Vec::leak(vec![0u64; (len as usize).div_ceil(8).max(1)]);
    words.as_ptr().expose_provenance() as u64
}
