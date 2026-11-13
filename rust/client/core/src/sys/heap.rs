// pxl-core/sys/heap.rs

//! The core's memory and its heap. The memory is what the program's runtime
//! sized it to before the core's first entry, never grown, so a full heap
//! stops the core. In address
//! order: the stack, the module's data, `.fixed` (sys/range.rs), then the
//! heap to the memory's end. The heap is the global allocator: xpute-kit's
//! slab of size classes over its buddy allocator of pages, whose blocks are
//! aligned to their own size, which bounds fragmentation. The slab borrows
//! and returns whole runs of pages, so neither level starves while the other
//! has room.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::slab::SlabMalloc;
use xpute_kit::alloc::ADDR_BITS;

use crate::sys::manifest::mem;

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
            let heap = page_up(fixed + mem::fixed_bytes as usize);
            xpute_kit::ensure!(heap + mem::min_heap_bytes as usize <= end, ENOMEM, heap, end);
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

/// A native core's memory is its region, which the native step gives.
#[cfg(not(target_arch = "wasm32"))]
fn given() -> (usize, usize) {
    xpute_kit::bug!(ENOSYS)
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
        page_up(fixed_at() + mem::fixed_bytes as usize)
    }

    fn end() -> usize {
        region().1
    }
}

/// A native core's heap waits for its region (the native step).
#[cfg_attr(target_arch = "wasm32", global_allocator)]
static HEAP: SlabMalloc<Span> = SlabMalloc::new();

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
