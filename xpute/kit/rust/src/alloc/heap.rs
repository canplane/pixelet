// xpute-kit/alloc/heap.rs

//! A heap over the range of memory a program gives it (`Range`): the slab of
//! size classes over the buddy allocator of pages, whose blocks are aligned
//! to their own size, which bounds fragmentation. The slab borrows and
//! returns whole runs of pages, so neither level starves while the other has
//! room. A request the range cannot hold is refused.
//!
//! A program that picks it declares it its global allocator. A process's
//! global allocator is every thread's, and the slab is one thread's, so each
//! call takes it under one lock: one word, spun on and then yielded on,
//! since a lock that allocates, as std's does on some systems, would enter
//! the allocator again.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use super::buddy_tree::Range;
use super::slab::SlabMalloc;

/// How many times a thread spins on the lock before it gives its time back
/// to the scheduler on each try instead.
const SPIN_CNT: u32 = 100;

pub struct HeapMalloc<R: Range> {
    slab: SlabMalloc<R>,
    /// Whether a thread holds the slab.
    lock: AtomicBool,
}

impl<R: Range> Default for HeapMalloc<R> {
    fn default() -> HeapMalloc<R> {
        HeapMalloc::new()
    }
}

impl<R: Range> HeapMalloc<R> {
    pub const fn new() -> HeapMalloc<R> {
        HeapMalloc {
            slab: SlabMalloc::new(),
            lock: AtomicBool::new(false),
        }
    }

    /// The slab, whose counters the program reads.
    pub fn slab(&self) -> &SlabMalloc<R> {
        &self.slab
    }

    /// The slab held, until the guard drops.
    fn take(&self) -> Guard<'_> {
        let mut spins = 0;
        while self
            .lock
            .compare_exchange_weak(false, true, Acquire, Relaxed)
            .is_err()
        {
            if spins < SPIN_CNT {
                spins += 1;
                core::hint::spin_loop();
            } else {
                std::thread::yield_now();
            }
        }
        Guard(&self.lock)
    }
}

/// The lock held, until it drops.
struct Guard<'a>(&'a AtomicBool);

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Release);
    }
}

// SAFETY: the slab's, each call alone while the lock is held.
unsafe impl<R: Range> GlobalAlloc for HeapMalloc<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _guard = self.take();
        // SAFETY: the caller's.
        unsafe { self.slab.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _guard = self.take();
        // SAFETY: the caller's.
        unsafe { self.slab.dealloc(ptr, layout) }
    }
}
