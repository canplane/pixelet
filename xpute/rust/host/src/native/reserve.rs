// xpute-host/native/reserve.rs

//! The core memory, reserved once from the system and never given back:
//! mapped as a WebAssembly memory is, so its pages are zero until written and
//! cost nothing until touched. Not through the system's allocator: C has no
//! aligned calloc, so `System.alloc_zeroed` at this alignment allocates and
//! then writes every byte to zero, the whole memory made resident at boot.

use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// The memory's alignment: a buddy block lies at an offset from the base that
/// is a multiple of its size, so a base on a large boundary keeps every block
/// aligned as far as anything asks.
pub const ALIGN: usize = 1 << 16;

/// The memory's first byte, 0 until it is reserved, and its bytes.
static AT: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

/// Maps `bytes` from the system, aligned to `ALIGN`: once, by `boot`, which
/// is once a process. Its address and its bytes.
pub fn reserve(bytes: usize) -> (usize, usize) {
    // Mapped a boundary's worth larger, and what lies outside the aligned
    // memory given back: the system aligns a mapping to its page, not to
    // this.
    let span = bytes + ALIGN;
    // SAFETY: an anonymous private mapping, which touches nothing that exists.
    let p = unsafe { libc::mmap(core::ptr::null_mut(), span, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0) };
    // No memory for the guest is no guest.
    assert!(p != libc::MAP_FAILED, "the system would not map a guest's {bytes} bytes");
    let p = p.cast::<u8>();
    let head = p.addr().next_multiple_of(ALIGN) - p.addr();
    let memory = p.wrapping_add(head);
    // SAFETY: the two ends of the mapping just made, outside the memory.
    unsafe {
        if head > 0 {
            libc::munmap(p.cast(), head);
        }
        if span - head > bytes {
            libc::munmap(memory.wrapping_add(bytes).cast(), span - head - bytes);
        }
    }
    let at = memory.expose_provenance();
    // The bytes before the address: a reader that sees the address sees its
    // span.
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
