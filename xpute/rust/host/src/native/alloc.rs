// xpute-host/native/alloc.rs

//! The process's allocator: the guest's thread's blocks in the core memory,
//! by xpute's slab over a buddy (xpute_guest::mem::heap), so what the guest
//! hands its host lies where the host reads; every other thread's the
//! system's. A block goes back to whichever holds its address, and the slab
//! is locked since a driver may free a guest's block on its own thread. The
//! `#[global_allocator]` static is the program's: one a crate graph.

use std::alloc::{GlobalAlloc, Layout, System};

use xpute_guest::global::on_marked_thread;
use xpute_guest::mem::heap::{heap_stat, Bounds, Heap, HeapStat};

use super::reserve::core;

/// The heap: from `heap_at` to the memory's end, read only once booted.
struct Range;

impl Bounds for Range {
    fn base() -> usize {
        let c = core().expect("a block is asked of the heap once the guest is booted");
        c.at + c.heap_at
    }
    fn end() -> usize {
        let c = core().expect("a block is asked of the heap once the guest is booted");
        c.at + c.bytes
    }
}

/// A lock that allocates nothing, since it is taken inside the allocator:
/// `os_unfair_lock` on Apple's systems, whose std `Mutex` may box.
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

/// Elsewhere std's `Mutex`, a futex word.
#[cfg(not(target_vendor = "apple"))]
struct SysLock(std::sync::Mutex<()>);

#[cfg(not(target_vendor = "apple"))]
impl SysLock {
    const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    fn hold<R>(&self, f: impl FnOnce() -> R) -> R {
        // A panic under the lock stops the guest; the slab is still the slab.
        let _held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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

pub fn stat() -> HeapStat {
    HEAP.with(heap_stat)
}

/// The allocator a program installs. Before boot no thread is the guest's,
/// so every block is the system's.
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
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let here = core().is_some_and(|c| c.holds(ptr));
        if here == on_marked_thread() {
            return if here {
                // SAFETY: as `dealloc`.
                HEAP.with(|a| unsafe { a.realloc(ptr, layout, new_size) })
            } else {
                // SAFETY: as `dealloc`.
                unsafe { System.realloc(ptr, layout, new_size) }
            };
        }
        // Grown across the line from where it was made: moved to this thread's side.
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
