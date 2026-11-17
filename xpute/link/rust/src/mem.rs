// xpute-link/mem.rs

//! The guest's memory, which every address on the link names, a u64 on every
//! platform (link/wit/types.wit `lent`): a module's offset into its own
//! memory, which begins at 0, so its offset is its pointer, or a native
//! guest's pointer, which a host linked into the same process reads as its
//! own. An address past what the platform's pointer holds is a bug.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::types::Lent;

/// The byte at `at`, as a pointer.
pub(crate) fn ptr(at: u64) -> *mut u8 {
    core::ptr::with_exposed_provenance_mut(usize::try_from(at).ok().or_bug(Errno::EFAULT))
}

/// The `len` bytes at `at`.
///
/// # Safety
/// They lie in the guest's memory, and nothing else writes them while the
/// slice lives.
pub unsafe fn bytes<'a>(at: u64, len: u32) -> &'a mut [u8] {
    // SAFETY: the caller's.
    unsafe { core::slice::from_raw_parts_mut(ptr(at), len as usize) }
}

/// The address of the value `v`, such as a word the guest lends its host to
/// store into.
pub fn addr<T>(v: &T) -> u64 {
    core::ptr::from_ref(v).expose_provenance() as u64
}

/// The range `s` as a lent buffer, where it lies and how many bytes it takes
/// (link/wit/types.wit `lent`). An empty range has no address, and lends
/// (0, 0), on the web and natively alike.
pub fn lent<T>(s: &[T]) -> Lent {
    let Some(first) = s.first() else {
        return Lent { buf: 0, nbyte: 0 };
    };
    let nbyte = u32::try_from(size_of_val(s)).ok().or_bug(Errno::EFAULT);
    Lent {
        buf: addr(first),
        nbyte,
    }
}

/// The `len` bytes at `at`.
///
/// # Safety
/// As `bytes`, and nothing writes them while the slice lives: what the guest
/// lent its host, until the answer.
pub unsafe fn read(at: u64, len: u32) -> &'static [u8] {
    // SAFETY: the caller's.
    unsafe { bytes(at, len) }
}

/// Writes `from` at `at`; `from` may lie in the memory.
///
/// # Safety
/// As `bytes`, and nothing else reads or writes them meanwhile: an answer
/// the host gives into what the guest lent, or words the caller alone writes.
pub unsafe fn write(at: u64, from: &[u8]) {
    // SAFETY: the caller's; the copy may overlap.
    unsafe {
        let to = bytes(at, from.len() as u32).as_mut_ptr();
        core::ptr::copy(from.as_ptr(), to, from.len());
    }
}

/// The little-endian word at `at`, aligned or not.
///
/// # Safety
/// As `read`.
pub unsafe fn word(at: u64) -> u32 {
    // SAFETY: the caller's.
    let w = unsafe { read(at, 4) };
    u32::from_le_bytes([w[0], w[1], w[2], w[3]])
}

/// Writes the little-endian word `v` at `at`, aligned or not.
///
/// # Safety
/// As `write`.
pub unsafe fn set_word(at: u64, v: u32) {
    // SAFETY: the caller's.
    unsafe { write(at, &v.to_le_bytes()) };
}

/// The aligned word at `at` as an atomic: a word that hands memory between
/// the guest and its host, such as a fence the guest reads while the
/// host answers it, which both sides touch only atomically.
fn atomic(at: u64) -> &'static core::sync::atomic::AtomicU32 {
    let p = ptr(at).cast::<u32>();
    xpute_kit::ensure!(p.is_aligned(), EFAULT, at);
    // SAFETY: a word of the guest's memory, aligned as just checked, which
    // both sides touch only atomically.
    unsafe { core::sync::atomic::AtomicU32::from_ptr(p) }
}

/// Loads such a word with acquire, so whatever the other side wrote before
/// it stored the word is seen.
pub fn load_acquire(at: u64) -> u32 {
    atomic(at).load(core::sync::atomic::Ordering::Acquire)
}

/// Stores `v` into such a word with release, so whatever this side wrote
/// before is seen by the side that loads it.
pub fn store_release(at: u64, v: u32) {
    atomic(at).store(v, core::sync::atomic::Ordering::Release);
}

#[cfg(test)]
#[path = "mem.test.rs"]
pub(crate) mod test;
