// xpute-runtime/native/alloc.rs

//! This is the process's allocator. A block asked for on the core's thread
//! comes from the core's memory, through xpute's slab over a buddy allocator
//! (xpute_core::mem::heap), so what the core hands its runtime lies where the
//! runtime can read it. A block asked for on any other thread comes from the
//! system. A freed block goes back to whichever of the two holds its address.
//! The slab is locked, because a driver may free one of the core's blocks on
//! the driver's own thread. The program declares the `#[global_allocator]`
//! static, since a crate graph may have only one.

use std::alloc::{GlobalAlloc, Layout, System};

use xpute_core::global::on_marked_thread;
use xpute_core::mem::heap::{heap_stat, Bound, Heap, HeapStat};

use super::reserve::core;

/// The heap's bounds run from `heap_at` to the memory's end. They are read only
/// once the core has booted.
struct Range;

impl Bound for Range {
    fn base() -> usize {
        let c = core().expect("a block is asked of the heap once the core is booted");
        c.at + c.heap_at
    }
    fn end() -> usize {
        let c = core().expect("a block is asked of the heap once the core is booted");
        c.at + c.bytes
    }
}

/// A lock that allocates nothing, because the allocator itself takes it. On
/// Apple's systems it is `os_unfair_lock`, since std's `Mutex` there may
/// allocate a box.
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

/// Elsewhere it is std's `Mutex`, a futex word, which allocates nothing.
#[cfg(not(target_vendor = "apple"))]
struct SysLock(std::sync::Mutex<()>);

#[cfg(not(target_vendor = "apple"))]
impl SysLock {
    const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    fn hold<R>(&self, f: impl FnOnce() -> R) -> R {
        // A panic under the lock stops the core, so the lock's poison is
        // ignored and the slab is used as it stands.
        let _guard = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f()
    }
}

struct Locked {
    lock: SysLock,
    slab: Heap<Range>,
}

// SAFETY: the slab is only reached holding the lock.
unsafe impl Sync for Locked {}

impl Locked {
    fn with<R>(&self, f: impl FnOnce(&Heap<Range>) -> R) -> R {
        self.lock.hold(|| f(&self.slab))
    }
}

static HEAP: Locked = Locked {
    lock: SysLock::new(),
    slab: Heap::new(),
};

pub(super) fn stat() -> HeapStat {
    HEAP.with(heap_stat)
}

/// A program installs this allocator. Before boot no thread is the core's, so
/// every block is the system's.
pub struct Alloc;

// SAFETY: every call into the slab holds the lock, and every block goes back
// to the allocator its address says gave it.
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if on_marked_thread() {
            // SAFETY: the caller's layout, passed on.
            HEAP.with(|a| unsafe { a.alloc(layout) })
        } else {
            // SAFETY: as above.
            unsafe { System.alloc(layout) }
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if core().is_some_and(|c| c.holds(ptr)) {
            // SAFETY: the caller's block, which the slab gave.
            HEAP.with(|a| unsafe { a.dealloc(ptr, layout) })
        } else {
            // SAFETY: the caller's block, which the system gave.
            unsafe { System.dealloc(ptr, layout) }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if on_marked_thread() {
            // SAFETY: as `alloc`.
            HEAP.with(|a| unsafe { a.alloc_zeroed(layout) })
        } else {
            // SAFETY: as `alloc`.
            unsafe { System.alloc_zeroed(layout) }
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_sz: usize) -> *mut u8 {
        let here = core().is_some_and(|c| c.holds(ptr));
        if here == on_marked_thread() {
            return if here {
                // SAFETY: as `dealloc`.
                HEAP.with(|a| unsafe { a.realloc(ptr, layout, new_sz) })
            } else {
                // SAFETY: as `dealloc`.
                unsafe { System.realloc(ptr, layout, new_sz) }
            };
        }
        // The block lies on the other side from this thread's allocator (the
        // core's memory or the system's), so growing it moves it to this side.
        // SAFETY: a layout the caller's allocation was valid under, resized.
        let grown =
            unsafe { self.alloc(Layout::from_size_align_unchecked(new_sz, layout.align())) };
        if !grown.is_null() {
            // SAFETY: two live blocks, the copy no longer than either.
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, grown, layout.size().min(new_sz));
                self.dealloc(ptr, layout);
            }
        }
        grown
    }
}
