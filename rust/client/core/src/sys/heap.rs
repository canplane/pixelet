// pxl-core/sys/heap.rs

//! The core's memory and its heap. The memory is what the program's runtime
//! sized it to before the core's first entry, never grown, so a full heap
//! stops the core. In address
//! order: the stack, the module's data, `.fixed` (sys/range.rs), then the
//! heap to the memory's end. The heap is the global allocator: xpute-kit's
//! slab of size classes over its buddy allocator of pages, whose blocks are
//! aligned to their own size, which bounds fragmentation. The slab borrows
//! and returns whole runs of pages, so neither level starves while the other
//! has room. A native core's memory is the region the heap reserves at the
//! process's first allocation (`given`), which it sets as the core's memory
//! (`xpute_link::mem::set`) before `link_init` lays the link, so every
//! allocation lies in it and every offset fits a u32. There the global
//! allocator is the process's, every thread's, so the heap is taken under
//! one lock.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::slab::SlabMalloc;
use xpute_kit::alloc::ADDR_BITS;

use crate::spec::manifest::mem;

/// The heap's counters, as its allocator reports them.
#[derive(Clone, Copy, Default)]
pub struct HeapStat {
    pub range: usize,
    /// The most ever live at once, not what is held now.
    pub peak: usize,
    pub live: usize,
    pub run: usize,
    pub slot: usize,
    /// The largest free block the tree lists. The range past the tree's
    /// current root is untouched rather than listed, so this is not the total
    /// free space.
    pub largest_free: usize,
    /// Advances each time a block is given back to the buddy, which is when a
    /// request refused before might now be served. A slot returned to a run
    /// still in use does not advance it.
    pub releases: u32,
    pub refusals: u32,
}

/// Every fixed range begins on a page, and so does the heap after them.
pub const PAGE_BYTES: u32 = 4096;

const fn page_up(at: usize) -> usize {
    at.next_multiple_of(PAGE_BYTES as usize)
}

/// Where `.fixed` begins and where the memory ends, once read.
static REGION: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];

/// Where `.fixed` begins and where the memory ends. The first call reads the
/// memory as the core was given it; one too small for the heap the core
/// counts on is a bug: the core cannot run.
fn region() -> (usize, usize) {
    match REGION[0].load(Relaxed) {
        0 => {
            let (fixed, end) = given();
            let heap = page_up(fixed + mem::FIXED_BYTES as usize);
            xpute_kit::ensure!(heap + mem::MIN_HEAP_BYTES as usize <= end, ENOMEM, heap, end);
            REGION[1].store(end, Relaxed);
            REGION[0].store(fixed, Relaxed);
            (fixed, end)
        }
        fixed => (fixed, REGION[1].load(Relaxed)),
    }
}

/// Where `.fixed` begins.
pub fn fixed_at() -> usize {
    region().0
}

/// The page past the module's data, and the memory's end.
#[cfg(target_arch = "wasm32")]
fn given() -> (usize, usize) {
    unsafe extern "C" {
        /// Where the module's data ends, as its linker places it.
        static __heap_base: u8;
    }
    let data_end = (&raw const __heap_base).expose_provenance();
    (page_up(data_end), core::arch::wasm32::memory_size(0) << 16)
}

/// A native core's region: the manifest's `mem.bytes`, reserved from the
/// system zeroed, as a module's memory starts, and set as the core's memory.
/// `.fixed` begins at its base, as nothing else lies in it. It is first asked
/// for inside the global allocator, which may not unwind, so a reservation the
/// system refuses stops the process.
#[cfg(not(target_arch = "wasm32"))]
fn given() -> (usize, usize) {
    use std::alloc::{GlobalAlloc, Layout, System};
    use xpute_link::mem::{set, Region};
    let len = mem::BYTES;
    let layout = Layout::from_size_align(len as usize, PAGE_BYTES as usize).ok();
    // SAFETY: a layout of nonzero size.
    let base = layout.map_or(core::ptr::null_mut(), |l| unsafe { System.alloc_zeroed(l) });
    if base.is_null() {
        std::process::abort();
    }
    set(Region { base, len });
    let at = base.expose_provenance();
    (at, at + len as usize)
}

/// The log2 of a page, the buddy's smallest block.
const PAGE_LOG2: usize = PAGE_BYTES.trailing_zeros() as usize;

/// 2 GiB, the largest power of two short of the whole address space. The
/// tree starts as one leaf and grows only as requests need it; `end` says
/// how much of the span is real memory.
const SPAN_LOG2: usize = ADDR_BITS - 1;

/// One bit for each possible parent node. The buddy checks this length on
/// the first request.
const SPLIT_BYTES: usize = (1 << (SPAN_LOG2 - PAGE_LOG2)) / 8;

/// The heap's range: from the page past `.fixed` to the memory's end.
pub struct Span(PhantomData<()>);

impl Range for Span {
    const MIN_LOG2: usize = PAGE_LOG2;
    const MAX_LOG2: usize = SPAN_LOG2;
    type Split = [u8; SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; SPLIT_BYTES];

    fn base() -> usize {
        page_up(fixed_at() + mem::FIXED_BYTES as usize)
    }

    fn end() -> usize {
        region().1
    }
}

#[cfg_attr(target_arch = "wasm32", global_allocator)]
static HEAP: SlabMalloc<Span> = SlabMalloc::new();

/// The heap as a native process's global allocator: the slab is one
/// thread's, and a process's threads all allocate, so each takes it under
/// the lock, the first reserving the region there. The lock is one word,
/// spun on and then yielded on, since a lock that allocates, as std's does on
/// some systems, would enter the allocator again.
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
        use core::sync::atomic::Ordering::Acquire;
        let mut spins = 0;
        while LOCK.compare_exchange_weak(false, true, Acquire, Relaxed).is_err() {
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
        LOCK.store(false, core::sync::atomic::Ordering::Release);
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
        region();
        // SAFETY: the caller's.
        unsafe { HEAP.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let _guard = Guard::take();
        // SAFETY: the caller's.
        unsafe { HEAP.dealloc(ptr, layout) }
    }
}

pub fn stat() -> HeapStat {
    HeapStat {
        range: HEAP.range_bytes(),
        peak: HEAP.peak(),
        live: HEAP.live(),
        run: HEAP.run_live(),
        slot: HEAP.slot_live(),
        largest_free: HEAP.largest_free(),
        releases: HEAP.releases(),
        refusals: HEAP.refusals(),
    }
}

/// `n` zeroed `T`s, or None when the heap refuses them.
pub fn zeroed<T: Copy + Default>(n: usize) -> Option<Box<[T]>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).ok()?;
    v.resize(n, T::default());
    Some(v.into_boxed_slice())
}
