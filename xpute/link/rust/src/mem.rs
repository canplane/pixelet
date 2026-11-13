// xpute-link/mem.rs

//! The guest's memory, which every offset on the link names: offset `at` is
//! the byte `at` past the memory's base. A module's memory begins at 0. A
//! native guest's is the one region it reserves as it starts (`set`), which a
//! host linked into the same process reads as its own.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// Where a native guest's memory lies.
#[derive(Clone, Copy)]
pub struct Region {
    pub base: *mut u8,
    pub len: u32,
}

#[cfg(target_arch = "wasm32")]
const BASE: usize = 0;

#[cfg(not(target_arch = "wasm32"))]
static REGION: core::sync::atomic::AtomicPtr<u8> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());
#[cfg(not(target_arch = "wasm32"))]
static LEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Sets the native guest's region, once, before anything reaches it: the
/// guest reserves it and its allocator hands out only memory inside it, so
/// every offset fits a u32. A second set is a bug.
#[cfg(not(target_arch = "wasm32"))]
pub fn set(r: Region) {
    use core::sync::atomic::Ordering::Relaxed;
    xpute_kit::ensure!(!r.base.is_null(), EINVAL);
    let first = REGION.compare_exchange(core::ptr::null_mut(), r.base, Relaxed, Relaxed);
    xpute_kit::ensure!(first.is_ok(), EALREADY);
    LEN.store(r.len, Relaxed);
}

/// The native guest's region.
#[cfg(not(target_arch = "wasm32"))]
pub fn region() -> Region {
    use core::sync::atomic::Ordering::Relaxed;
    Region {
        base: REGION.load(Relaxed),
        len: LEN.load(Relaxed),
    }
}

/// The region's base; nothing reaches the memory before it is set.
#[cfg(not(target_arch = "wasm32"))]
fn base() -> usize {
    let base = region().base;
    xpute_kit::ensure!(!base.is_null(), ENOTRECOVERABLE);
    base.expose_provenance()
}

#[cfg(target_arch = "wasm32")]
fn base() -> usize {
    BASE
}

/// The `len` bytes at `at`. A native guest's are held to its region: a range
/// past it is a bug, as a module's memory traps past its end.
///
/// # Safety
/// Nothing else writes them while the slice lives.
pub unsafe fn bytes<'a>(at: u32, len: u32) -> &'a mut [u8] {
    #[cfg(not(target_arch = "wasm32"))]
    xpute_kit::ensure!(
        u64::from(at) + u64::from(len) <= u64::from(region().len),
        EFAULT,
        at,
        len
    );
    let p = core::ptr::with_exposed_provenance_mut::<u8>(base() + at as usize);
    // SAFETY: the caller's.
    unsafe { core::slice::from_raw_parts_mut(p, len as usize) }
}

/// The offset of `p` in the guest's memory; an address outside it is a bug,
/// never a truncated offset.
pub fn off(p: *const u8) -> u32 {
    let off = p.expose_provenance().wrapping_sub(base());
    u32::try_from(off).ok().or_bug(Errno::EFAULT)
}

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
