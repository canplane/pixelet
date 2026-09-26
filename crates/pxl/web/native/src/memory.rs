// pxl-native/memory.rs

//! The kernel's memory on a device: one region taken from the system at the
//! first request and never given back, the base every address the kernel
//! names is an offset from (xpute_runtime::mem::base), and the allocator over
//! it — the slab over a buddy the module has (pxl-wasm), so a `Vec` the kernel
//! hands the shell lies in the memory the shell reads.
//!
//! Unlike the module's, this allocator is the whole library's, and a device's
//! GPU driver calls back on threads of its own; a lock makes the slab — built
//! for one thread — sound for them. The kernel's thread takes it uncontended.
//!
//! Being the whole library's, it is wgpu's as well: what the device keeps on
//! the CPU for its objects lies in the region beside the kernel's own blocks
//! and counts in the heap's row, which a page's module has no part of. It
//! cannot be told apart — an allocator is not told who asks, and wgpu asks on
//! the kernel's thread too — so a device's row reads higher than a page's by
//! what its driver holds.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{GlobalAlloc, Layout};

use pxl_kernel::host::HeapStat;
use pxl_kernel::mem::heap::{heap_stat, HeapBounds, KernelAlloc};
use pxl_kernel::mem::section;
use pxl_kernel::mem::spec::{GLOBAL_BASE, PHONE_BYTES};

/// The memory the kernel is given: a phone's reservation, mem.json's, as a
/// page on a phone reserves.
pub const GIVEN: u32 = PHONE_BYTES;

/// Where the bracket's own data ends: at mem.json's GLOBAL_BASE, past the
/// rings' front (pxl-kernel door/layout.rs). A device's library keeps its
/// statics where the linker put them, so nothing of its own lies in the
/// memory above the front.
pub const DATA_END: u32 = GLOBAL_BASE;

/// The region's alignment: a buddy block lies at an offset from the base that
/// is a multiple of its size, so a base on a large boundary keeps every block
/// aligned as far as anything asks.
const REGION_ALIGN: usize = 1 << 16;

/// The region's first byte, 0 before the first request.
static REGION: AtomicUsize = AtomicUsize::new(0);

/// The region, taken on the first call and made the memory's base: mapped
/// from the system as a WebAssembly memory is, so its pages are zero until
/// written and cost nothing until touched. Not through the system allocator:
/// C has no aligned calloc, so `System.alloc_zeroed` at this alignment
/// allocates and then writes every byte to zero — the whole region made
/// resident at boot, which a phone killed the app for.
pub fn region() -> usize {
    let at = REGION.load(Ordering::Acquire);
    if at != 0 {
        return at;
    }
    let bytes = GIVEN as usize;
    // Mapped a boundary's worth larger, and what lies outside the aligned
    // region given back: the system aligns a mapping to its page, not to this.
    let span = bytes + REGION_ALIGN;
    // SAFETY: an anonymous private mapping, which touches nothing that exists.
    let p = unsafe { libc::mmap(core::ptr::null_mut(), span, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0) };
    if p == libc::MAP_FAILED {
        // No memory for the kernel is no kernel; nothing can be reported
        // from inside the allocator that would need it.
        std::process::abort();
    }
    let p = p.cast::<u8>();
    let head = p.addr().next_multiple_of(REGION_ALIGN) - p.addr();
    let region = p.wrapping_add(head);
    // SAFETY: the two ends of the mapping just made, outside the region.
    unsafe {
        if head > 0 {
            libc::munmap(p.cast(), head);
        }
        if span - head > bytes {
            libc::munmap(region.wrapping_add(bytes).cast(), span - head - bytes);
        }
    }
    let addr = region.expose_provenance();
    match REGION.compare_exchange(0, addr, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            xpute_runtime::mem::base::set_base(addr, bytes);
            addr
        }
        Err(won) => {
            // SAFETY: the region this call mapped, which nothing has seen.
            unsafe { libc::munmap(region.cast(), bytes) };
            won
        }
    }
}

/// Where the heap lies: from the page past the last fixed range to the end of
/// the region, as the module's is to the end of its memory.
struct RegionBounds;

impl HeapBounds for RegionBounds {
    fn base() -> usize {
        region() + section::heap_at(DATA_END) as usize
    }
    fn end() -> usize {
        region() + GIVEN as usize
    }
}

/// The system's lock for this (`os_unfair_lock`, libSystem's): it allocates
/// nothing, where std's `Mutex` may box its pthread mutex on first use —
/// inside the allocator, a call back into itself — and it lends its holder
/// the waiter's priority, where a spin lock leaves a low-priority holder
/// preempted by the high-priority thread spinning for it.
#[repr(C)]
struct UnfairLock(UnsafeCell<u32>);

extern "C" {
    fn os_unfair_lock_lock(lock: *mut u32);
    fn os_unfair_lock_unlock(lock: *mut u32);
}

/// The slab behind the system's lock.
pub struct Locked {
    lock: UnfairLock,
    slab: KernelAlloc<RegionBounds>,
}

// SAFETY: the slab is only reached holding the lock.
unsafe impl Sync for Locked {}

impl Locked {
    fn with<R>(&self, f: impl FnOnce(&KernelAlloc<RegionBounds>) -> R) -> R {
        // SAFETY: a lock in a static, never moved, locked and unlocked on the
        // same thread.
        unsafe { os_unfair_lock_lock(self.lock.0.get()) };
        let r = f(&self.slab);
        // SAFETY: as above.
        unsafe { os_unfair_lock_unlock(self.lock.0.get()) };
        r
    }

    /// The allocator's reading, for the ledger and the heap's row.
    pub fn stat(&self) -> HeapStat {
        self.with(heap_stat)
    }
}

// SAFETY: every call into the slab holds the lock.
unsafe impl GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout, passed on.
        self.with(|a| unsafe { a.alloc(layout) })
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's block, passed on.
        self.with(|a| unsafe { a.dealloc(ptr, layout) })
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as `alloc`.
        self.with(|a| unsafe { a.alloc_zeroed(layout) })
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: as `dealloc`.
        self.with(|a| unsafe { a.realloc(ptr, layout, new_size) })
    }
}

/// The library's one allocator.
#[global_allocator]
pub static ALLOC: Locked = Locked {
    lock: UnfairLock(UnsafeCell::new(0)),
    slab: KernelAlloc::new(),
};
