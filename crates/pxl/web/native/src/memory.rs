// pxl-native/memory.rs

//! The kernel's memory on a device: one region taken from the system at the
//! first request and never given back, the base every address the kernel
//! names is an offset from (xpute_runtime::mem::base), and the allocator over
//! it — the slab over a buddy the module has (pxl-wasm), so a `Vec` the kernel
//! hands the shell lies in the memory the shell reads.
//!
//! Unlike the module's, this allocator is the whole library's, and the
//! library runs on more threads than the kernel's: an image decoded off it,
//! the GPU driver's callbacks. What the kernel's thread asks for is placed in
//! the region; what any other thread asks for is the system's, as a page's
//! bitmaps are the browser's and not the module's. A block goes back to
//! whichever holds its address.
//!
//! A block of the region can still be let go on another thread — wgpu drops
//! what the kernel made where its callbacks run — so the slab, built for one
//! thread, is behind a lock; the kernel's thread takes it uncontended. What
//! wgpu keeps on the CPU for its objects is made on the kernel's thread, lies
//! in the region and counts in the heap's row, which a page's module has no
//! part of: a device's row reads higher than a page's by what its driver
//! holds.

use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{GlobalAlloc, Layout, System};

use pxl_kernel::host::HeapStat;
use pxl_kernel::mem::heap::{heap_stat, HeapBounds, KernelAlloc};

use crate::thread::on_kernel_thread;
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

/// The system's lock for this. It must allocate nothing — it is taken
/// inside the allocator, where an allocation is a call back into itself —
/// and should lend its holder the waiter's priority, where a spin lock leaves
/// a low-priority holder preempted by the high-priority thread spinning for
/// it. On an iPhone that is `os_unfair_lock`, libSystem's, since std's
/// `Mutex` there may box its pthread mutex on first use.
#[cfg(target_vendor = "apple")]
#[repr(C)]
struct SysLock(core::cell::UnsafeCell<u32>);

#[cfg(target_vendor = "apple")]
extern "C" {
    fn os_unfair_lock_lock(lock: *mut u32);
    fn os_unfair_lock_unlock(lock: *mut u32);
}

#[cfg(target_vendor = "apple")]
impl SysLock {
    const fn new() -> Self {
        Self(core::cell::UnsafeCell::new(0))
    }

    fn hold<R>(&self, f: impl FnOnce() -> R) -> R {
        // SAFETY: a lock in a static, never moved, locked and unlocked on the
        // same thread.
        unsafe { os_unfair_lock_lock(self.0.get()) };
        let r = f();
        // SAFETY: as above.
        unsafe { os_unfair_lock_unlock(self.0.get()) };
        r
    }
}

/// On Android std's `Mutex` is the lock: a futex word, which allocates
/// nothing.
#[cfg(not(target_vendor = "apple"))]
struct SysLock(std::sync::Mutex<()>);

#[cfg(not(target_vendor = "apple"))]
impl SysLock {
    const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    fn hold<R>(&self, f: impl FnOnce() -> R) -> R {
        // A panic under the lock stops the kernel; a poisoned lock is still
        // the slab's.
        let _held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f()
    }
}

/// The slab behind the system's lock.
pub struct Locked {
    lock: SysLock,
    slab: KernelAlloc<RegionBounds>,
}

// SAFETY: the slab is only reached holding the lock.
unsafe impl Sync for Locked {}

impl Locked {
    fn with<R>(&self, f: impl FnOnce(&KernelAlloc<RegionBounds>) -> R) -> R {
        self.lock.hold(|| f(&self.slab))
    }

    /// The allocator's reading, for the profiler and the heap's row.
    pub fn stat(&self) -> HeapStat {
        self.with(heap_stat)
    }
}

/// Whether `ptr` lies in the region, where the slab gave it.
fn in_region(ptr: *mut u8) -> bool {
    let at = REGION.load(Ordering::Acquire);
    at != 0 && (at..at + GIVEN as usize).contains(&ptr.addr())
}

// SAFETY: every call into the slab holds the lock, and every block goes back
// to the allocator its address says gave it.
unsafe impl GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if on_kernel_thread() {
            // SAFETY: the caller's layout, passed on.
            self.with(|a| unsafe { a.alloc(layout) })
        } else {
            // SAFETY: as above.
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if in_region(ptr) {
            // SAFETY: the caller's block, which the slab gave.
            self.with(|a| unsafe { a.dealloc(ptr, layout) })
        } else {
            // SAFETY: the caller's block, which the system gave.
            unsafe { System.dealloc(ptr, layout) }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if on_kernel_thread() {
            // SAFETY: as `alloc`.
            self.with(|a| unsafe { a.alloc_zeroed(layout) })
        } else {
            // SAFETY: as `alloc`.
            unsafe { System.alloc_zeroed(layout) }
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let here = in_region(ptr);
        if here == on_kernel_thread() {
            return if here {
                // SAFETY: as `dealloc`.
                self.with(|a| unsafe { a.realloc(ptr, layout, new_size) })
            } else {
                // SAFETY: as `dealloc`.
                unsafe { System.realloc(ptr, layout, new_size) }
            };
        }
        // A block that grows on the other side of the line from where it was
        // made: moved to where this thread's blocks are.
        // SAFETY: a layout the caller's allocation was valid under, resized.
        let grown = unsafe { self.alloc(Layout::from_size_align_unchecked(new_size, layout.align())) };
        if !grown.is_null() {
            // SAFETY: two live blocks, the copy no longer than either.
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, grown, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
        }
        grown
    }
}

/// The library's one allocator.
#[global_allocator]
pub static ALLOC: Locked = Locked {
    lock: SysLock::new(),
    slab: KernelAlloc::new(),
};
