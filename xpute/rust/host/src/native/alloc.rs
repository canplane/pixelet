// xpute-host/native/alloc.rs

//! The process's allocator, for a program that links its guest in: what the
//! guest's thread asks for is placed in the core memory, by xpute's slab over
//! a buddy (xpute_guest::mem::heap), so a block the guest hands its host lies
//! in the memory the host reads; what any other thread asks for is the
//! system's, as a page's bitmaps are the browser's and not the module's. A
//! block goes back to whichever holds its address.
//!
//! A block of the memory can still be let go on another thread — a driver
//! drops what the guest made where its callbacks run — so the slab, built for
//! one thread, is behind a lock, which the guest's thread takes uncontended.
//!
//! The heap's bounds and the line between the two are one reading of the
//! reserved memory: from the plan's `heap_at` to the memory's end, never a
//! second statement of its size, so no block of the heap can lie past what
//! the line calls the memory's.
//!
//! The `#[global_allocator]` static is the program's own, in the crate it
//! builds last: one is allowed a crate graph, and a library that declared it
//! would install it into every test binary that links the library.

use core::sync::atomic::{AtomicUsize, Ordering};
use std::alloc::{GlobalAlloc, Layout, System};

use xpute_guest::global::on_marked_thread;
use xpute_guest::mem::heap::{heap_stat, Bounds, Heap, HeapStat};

use super::reserve;

/// The heap's first byte's offset, the plan's `heap_at`.
static HEAP_AT: AtomicUsize = AtomicUsize::new(0);

/// Set by `boot`, before the guest's thread can ask for a block.
pub fn bound(heap_at: usize) {
    HEAP_AT.store(heap_at, Ordering::Release);
}

/// The heap: from `heap_at` to the end of the reserved memory.
struct Core;

impl Bounds for Core {
    fn base() -> usize {
        reserve::bounds().0 + HEAP_AT.load(Ordering::Acquire)
    }
    fn end() -> usize {
        let (at, bytes) = reserve::bounds();
        at + bytes
    }
}

/// The system's lock for the slab. It must allocate nothing — it is taken
/// inside the allocator, where an allocation is a call back into itself —
/// and should lend its holder the waiter's priority, where a spin lock leaves
/// a low-priority holder preempted by the high-priority thread spinning for
/// it. On Apple's systems that is `os_unfair_lock`, libSystem's, since std's
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

/// Elsewhere std's `Mutex` is the lock: on Linux and Android a futex word,
/// which allocates nothing.
#[cfg(not(target_vendor = "apple"))]
struct SysLock(std::sync::Mutex<()>);

#[cfg(not(target_vendor = "apple"))]
impl SysLock {
    const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    fn hold<R>(&self, f: impl FnOnce() -> R) -> R {
        // A panic under the lock stops the guest; a poisoned lock is still
        // the slab's.
        let _held = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f()
    }
}

/// The slab behind the system's lock.
struct Locked {
    lock: SysLock,
    slab: Heap<Core>,
}

// SAFETY: the slab is only reached holding the lock.
unsafe impl Sync for Locked {}

impl Locked {
    fn with<R>(&self, f: impl FnOnce(&Heap<Core>) -> R) -> R {
        self.lock.hold(|| f(&self.slab))
    }
}

/// The one heap, whichever `Alloc` is installed.
static HEAP: Locked = Locked {
    lock: SysLock::new(),
    slab: Heap::new(),
};

/// The heap's reading, for a profiler.
pub fn stat() -> HeapStat {
    HEAP.with(heap_stat)
}

/// The allocator a program installs as its `#[global_allocator]`. It holds
/// nothing: every `Alloc` is the one heap. Before `boot` marks the guest's
/// thread no thread is the guest's, and every block is the system's: this
/// reads "no thread marked" the other way from a guest asking whether it may
/// touch its `Global`s (`xpute_guest::global`), since there is no memory yet
/// to place a block in.
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
        if reserve::holds(ptr) {
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
        let here = reserve::holds(ptr);
        if here == on_marked_thread() {
            return if here {
                // SAFETY: as `dealloc`.
                HEAP.with(|a| unsafe { a.realloc(ptr, layout, new_size) })
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
