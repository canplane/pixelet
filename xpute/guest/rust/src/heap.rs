// xpute-guest/heap.rs

//! The guest's heap, its global allocator: xpute-kit's slab of size classes
//! over its buddy allocator of pages, whose blocks are aligned to their own
//! size, which bounds fragmentation. The slab borrows and returns whole runs
//! of pages, so neither level starves while the other has room.
//!
//! The heap lies in the guest's memory as its runtime sized it (xpute-link
//! mem.rs `region`), past the program's share: a module's share begins on the
//! page past its data, past the stack the module's linker lays first; a native
//! guest's at its region's base. The heap runs from the page past the share to
//! the memory's end. The memory is never grown, so a full heap refuses.
//!
//! A native guest's global allocator is the process's, every thread's, so the
//! heap is taken under one lock, and its first call, from whatever thread
//! allocates first, has the runtime reserve the region.

use core::marker::PhantomData;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::slab::SlabMalloc;
use xpute_kit::alloc::ADDR_BITS;
use xpute_link::mem;

/// The buddy's smallest block. The share begins on one, and so does the heap
/// past it.
pub const PAGE_SZ: usize = 4096;

const fn page_up(at: usize) -> usize {
    at.next_multiple_of(PAGE_SZ)
}

/// Where the share begins, where the heap begins and where the memory ends,
/// once read.
static LAYOUT: [AtomicUsize; 3] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];

/// Where the share begins, where the heap begins and where the memory ends.
/// The first call reads the memory as the runtime sized it; a share past its
/// end is a bug. Two first calls at once read the same memory.
fn layout() -> [usize; 3] {
    match LAYOUT[0].load(Acquire) {
        0 => {
            let r = mem::region();
            let share = share_from(r);
            let (base, end) = (page_up(share + r.share_sz as usize), r.base.expose_provenance() + r.len as usize);
            xpute_kit::ensure!(base <= end, ENOMEM, base, end);
            LAYOUT[2].store(end, Relaxed);
            LAYOUT[1].store(base, Relaxed);
            LAYOUT[0].store(share, Release);
            [share, base, end]
        }
        share => [share, LAYOUT[1].load(Relaxed), LAYOUT[2].load(Relaxed)],
    }
}

/// The page past the module's data.
#[cfg(target_arch = "wasm32")]
fn share_from(_: mem::Region) -> usize {
    unsafe extern "C" {
        /// Where the module's data ends, as its linker places it.
        static __heap_base: u8;
    }
    page_up((&raw const __heap_base).expose_provenance())
}

/// The region's base, as nothing else lies in it.
#[cfg(not(target_arch = "wasm32"))]
fn share_from(r: mem::Region) -> usize {
    r.base.expose_provenance()
}

/// Where the program's share begins: as many bytes as it gave its runtime,
/// zeroed, its own for the guest's life.
pub fn share_at() -> usize {
    layout()[0]
}

/// The log2 of a page.
const PAGE_LOG2: usize = PAGE_SZ.trailing_zeros() as usize;

/// 2 GiB, the largest power of two short of the whole address space. The
/// tree starts as one leaf and grows only as requests need it; `end` says
/// how much of the span is real memory.
const SPAN_LOG2: usize = ADDR_BITS - 1;

/// One bit for each possible parent node. The buddy checks this length on
/// the first request.
const SPLIT_SZ: usize = (1 << (SPAN_LOG2 - PAGE_LOG2)) / 8;

/// The heap's range: from the page past the share to the memory's end.
pub struct Span(PhantomData<()>);

impl Range for Span {
    const MIN_LOG2: usize = PAGE_LOG2;
    const MAX_LOG2: usize = SPAN_LOG2;
    type Split = [u8; SPLIT_SZ];
    const SPLIT_INIT: Self::Split = [0; SPLIT_SZ];

    fn base() -> usize {
        layout()[1]
    }

    fn end() -> usize {
        layout()[2]
    }
}

/// The heap. Its counters are the program's to read; a native guest's
/// allocator is `Locked`.
#[cfg_attr(target_arch = "wasm32", global_allocator)]
pub static HEAP: SlabMalloc<Span> = SlabMalloc::new();

/// The heap as a native process's global allocator: the slab is one
/// thread's, and a process's threads all allocate, so each takes it under
/// the lock. The lock is one word, spun on and then yielded on, since a lock
/// that allocates, as std's does on some systems, would enter the allocator
/// again.
#[cfg(not(target_arch = "wasm32"))]
struct Locked;

/// Whether a thread holds the native heap.
#[cfg(not(target_arch = "wasm32"))]
static LOCK: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// How many times a thread spins on the native heap's lock before it gives
/// its time back to the scheduler on each try instead.
#[cfg(not(target_arch = "wasm32"))]
const SPINS: u32 = 100;

/// The native heap held, until it drops.
#[cfg(not(target_arch = "wasm32"))]
struct Guard;

#[cfg(not(target_arch = "wasm32"))]
impl Guard {
    fn take() -> Guard {
        let mut spins = 0;
        while LOCK
            .compare_exchange_weak(false, true, Acquire, Relaxed)
            .is_err()
        {
            if spins < SPINS {
                spins += 1;
                core::hint::spin_loop();
            } else {
                std::thread::yield_now();
            }
        }
        Guard
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Release);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static NATIVE: Locked = Locked;

// SAFETY: the slab's, each call alone under the lock.
#[cfg(not(target_arch = "wasm32"))]
unsafe impl core::alloc::GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let _guard = Guard::take();
        // SAFETY: the caller's.
        unsafe { core::alloc::GlobalAlloc::alloc(&HEAP, layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let _guard = Guard::take();
        // SAFETY: the caller's.
        unsafe { core::alloc::GlobalAlloc::dealloc(&HEAP, ptr, layout) }
    }
}
