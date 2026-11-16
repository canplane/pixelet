// xpute-kit/alloc/heap.rs

//! A heap over the range of memory a program gives it (`Range`): the slab of
//! size classes over the buddy allocator of pages, whose blocks are aligned
//! to their own size, which bounds fragmentation. The slab borrows and
//! returns whole runs of pages, so neither level starves while the other has
//! room. A request the range cannot hold is refused.
//!
//! A program that picks it declares it its global allocator. A process's
//! global allocator is every thread's, and the slab is one thread's, so where
//! the target runs threads each call takes it under one lock (`Lock`): one
//! word, spun on and then yielded on, since a lock that allocates, as std's
//! does on some systems, would enter the allocator again. A WebAssembly
//! target without the `atomics` feature runs no threads, and takes none, as
//! std's and dlmalloc's allocators take none there.

use core::alloc::{GlobalAlloc, Layout};

use super::buddy_tree::Range;
use super::slab::SlabMalloc;

pub struct HeapMalloc<R: Range> {
    slab: SlabMalloc<R>,
    lock: Lock,
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
            lock: Lock::new(),
        }
    }

    /// The slab, whose counters the program reads.
    pub fn slab(&self) -> &SlabMalloc<R> {
        &self.slab
    }
}

// SAFETY: the slab's, each call alone while the lock is held.
unsafe impl<R: Range> GlobalAlloc for HeapMalloc<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _guard = self.lock.take();
        // SAFETY: the caller's.
        unsafe { self.slab.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _guard = self.lock.take();
        // SAFETY: the caller's.
        unsafe { self.slab.dealloc(ptr, layout) }
    }
}

/// The slab's lock, on a target that runs threads: whether a thread holds
/// the slab.
#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
struct Lock(core::sync::atomic::AtomicBool);

#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
impl Lock {
    /// How many times a thread spins on the lock before it gives its time
    /// back to the scheduler on each try instead.
    const SPIN_CNT: u32 = 100;

    const fn new() -> Lock {
        Lock(core::sync::atomic::AtomicBool::new(false))
    }

    /// The slab held, until the guard drops.
    fn take(&self) -> Guard<'_> {
        use core::sync::atomic::Ordering::{Acquire, Relaxed};
        let mut spins = 0;
        while self
            .0
            .compare_exchange_weak(false, true, Acquire, Relaxed)
            .is_err()
        {
            if spins < Lock::SPIN_CNT {
                spins += 1;
                core::hint::spin_loop();
            } else {
                std::thread::yield_now();
            }
        }
        Guard(&self.0)
    }
}

/// The lock held, until it drops.
#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
struct Guard<'a>(&'a core::sync::atomic::AtomicBool);

#[cfg(not(all(target_family = "wasm", not(target_feature = "atomics"))))]
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, core::sync::atomic::Ordering::Release);
    }
}

/// The slab's lock, on a target that runs no threads: none, the slab held by
/// the one thread there is.
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
struct Lock;

#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
impl Lock {
    const fn new() -> Lock {
        Lock
    }

    /// The slab, which the one thread holds already.
    fn take(&self) -> Guard {
        Guard
    }
}

/// Nothing held.
#[cfg(all(target_family = "wasm", not(target_feature = "atomics")))]
struct Guard;
