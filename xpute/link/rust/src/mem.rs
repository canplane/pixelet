// xpute-link/mem.rs

//! The guest's memory, which every offset on the link names: offset `at` is
//! the byte `at` past the memory's base. A module's memory begins at 0. A
//! native guest's is the region its runtime reserves (`region`), which a host
//! linked into the same process reads as its own.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

// The runtime's (xpute.h `xpute_mem`): the memory's first byte, its size
// through `len` and the program's share of it through `share_sz`. A module
// imports it under `env` from its page's runtime.
#[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "env"))]
unsafe extern "C" {
    fn xpute_mem(len: *mut u32, share_sz: *mut u32) -> *mut u8;
}

/// Where the guest's memory lies, as its runtime sized it before the
/// guest's first crossing: its first byte, its size, and the bytes the
/// program keeps for itself, which the guest's heap leaves before its own.
#[derive(Clone, Copy)]
pub struct Region {
    pub base: *mut u8,
    pub len: u32,
    pub share_sz: u32,
}

/// The guest's memory, as its runtime answers it. A native guest's region is
/// reserved by the first call, from any thread.
pub fn region() -> Region {
    let (mut len, mut share_sz) = (0, 0);
    // SAFETY: two words to write.
    let base = unsafe { xpute_mem(&raw mut len, &raw mut share_sz) };
    Region {
        base,
        len,
        share_sz,
    }
}

#[cfg(target_arch = "wasm32")]
fn base() -> usize {
    0
}

/// A native guest's region's base and size, once read: the region never
/// moves once reserved.
#[cfg(not(target_arch = "wasm32"))]
static BASE: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
#[cfg(not(target_arch = "wasm32"))]
static LEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// A native guest's region's base and size. The first call asks the runtime;
/// two first calls at once read the same region.
#[cfg(not(target_arch = "wasm32"))]
fn kept() -> (usize, u32) {
    use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
    match BASE.load(Acquire) {
        0 => {
            let r = region();
            let base = r.base.expose_provenance();
            LEN.store(r.len, Relaxed);
            BASE.store(base, Release);
            (base, r.len)
        }
        base => (base, LEN.load(Relaxed)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn base() -> usize {
    kept().0
}

/// The `len` bytes at `at`. A native guest's are held to its region: a range
/// past it is a bug, as a module's memory traps past its end.
///
/// # Safety
/// Nothing else writes them while the slice lives.
pub unsafe fn bytes<'a>(at: u32, len: u32) -> &'a mut [u8] {
    held(at, len);
    let p = core::ptr::with_exposed_provenance_mut::<u8>(base() + at as usize);
    // SAFETY: the caller's.
    unsafe { core::slice::from_raw_parts_mut(p, len as usize) }
}

/// The offset of the value `v` in the guest's memory, such as a word the
/// guest lends its host to store into; an address whose offset does not fit
/// a u32 is a bug, never truncated.
pub fn off<T>(v: &T) -> u32 {
    let off = core::ptr::from_ref(v)
        .expose_provenance()
        .wrapping_sub(base());
    u32::try_from(off).ok().or_bug(Errno::EFAULT)
}

/// Where the range `s` lies in the guest's memory and how many bytes it
/// takes, as a lent span (link/wit/types.wit `lent`). An empty range has no
/// address, and lends (0, 0), on a page and on a device alike; any other lies
/// whole in the memory, a native guest's in its region (`bytes`).
pub fn span<T>(s: &[T]) -> (u32, u32) {
    let Some(first) = s.first() else {
        return (0, 0);
    };
    let (at, len) = (off(first), size_of_val(s) as u32);
    held(at, len);
    (at, len)
}

/// Holds the `len` bytes at `at` to a native guest's region: a range past it
/// is a bug, as a module's memory traps past its end, which holds a module's.
#[cfg(not(target_arch = "wasm32"))]
fn held(at: u32, len: u32) {
    xpute_kit::ensure!(
        u64::from(at) + u64::from(len) <= u64::from(kept().1),
        EFAULT,
        at,
        len
    );
}

#[cfg(target_arch = "wasm32")]
fn held(_: u32, _: u32) {}

/// The `len` bytes at `at`.
///
/// # Safety
/// Nothing writes them while the slice lives: what the guest lent its
/// host, until the answer.
pub unsafe fn read(at: u32, len: u32) -> &'static [u8] {
    // SAFETY: the caller's.
    unsafe { bytes(at, len) }
}

/// Writes `from` at `at`; `from` may lie in the memory.
///
/// # Safety
/// Nothing else reads or writes the span meanwhile: an answer the host
/// gives into what the guest lent, or words the caller alone writes.
pub unsafe fn write(at: u32, from: &[u8]) {
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
pub unsafe fn word(at: u32) -> u32 {
    // SAFETY: the caller's.
    let w = unsafe { read(at, 4) };
    u32::from_le_bytes([w[0], w[1], w[2], w[3]])
}

/// Writes the little-endian word `v` at `at`, aligned or not.
///
/// # Safety
/// As `write`.
pub unsafe fn set_word(at: u32, v: u32) {
    // SAFETY: the caller's.
    unsafe { write(at, &v.to_le_bytes()) };
}

/// The aligned word at `at` as an atomic: a word that hands memory between
/// the guest and its host, such as a fence the guest reads while the
/// host answers it, which both sides touch only atomically.
fn atomic(at: u32) -> &'static core::sync::atomic::AtomicU32 {
    // SAFETY: a word both sides touch only atomically.
    let p = unsafe { bytes(at, 4) }.as_mut_ptr().cast::<u32>();
    xpute_kit::ensure!(p.is_aligned(), EFAULT, at);
    // SAFETY: in the memory and aligned, as just checked.
    unsafe { core::sync::atomic::AtomicU32::from_ptr(p) }
}

/// Loads such a word with acquire, so whatever the other side wrote before
/// it stored the word is seen.
pub fn load_acquire(at: u32) -> u32 {
    atomic(at).load(core::sync::atomic::Ordering::Acquire)
}

/// Stores `v` into such a word with release, so whatever this side wrote
/// before is seen by the side that loads it.
pub fn store_release(at: u32, v: u32) {
    atomic(at).store(v, core::sync::atomic::Ordering::Release);
}

#[cfg(test)]
#[path = "mem.test.rs"]
pub(crate) mod test;
