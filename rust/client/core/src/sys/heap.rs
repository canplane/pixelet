// pxl-core/sys/heap.rs

//! The core's memory, fixed at the manifest's `mem.sz`: on a page the
//! module's memory, which its link holds at that size from its start to its
//! end (build.rs), and on a device a block the core reserves of the process
//! at that size (`mem_at`). `.fixed`, the core's fixed ranges
//! (sys/range.rs), begins on the page past the module's data, or at the
//! block's base, and the heap on the page past `.fixed` and runs to the
//! memory's end. The heap is
//! xpute-kit's (`HeapMalloc`), the core's global allocator, which the core's
//! terminal crates declare (lib.rs on a page, pxl-runtime on a device). The
//! memory is never grown, so a full heap refuses.

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::heap::HeapMalloc;
use xpute_kit::alloc::ADDR_BITS;

use crate::spec::manifest::mem;

/// The heap's counters, as its allocator reports them.
#[derive(Clone, Copy, Default)]
pub(crate) struct HeapStat {
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

/// The buddy's smallest block. Every fixed range begins on one, as the heap
/// after them does.
pub(crate) const PAGE_BYTES: u32 = 4096;

const fn page_up(at: usize) -> usize {
    at.next_multiple_of(PAGE_BYTES as usize)
}

/// Where the memory begins: a module's at 0.
#[cfg(target_arch = "wasm32")]
fn mem_at() -> usize {
    0
}

/// Where the memory begins: the block of `mem.sz` bytes on a page, zeroed,
/// which the first call reserves of the system, from any thread, and which
/// lasts the process's life. It is first asked for inside the global
/// allocator, which may not unwind, so a reservation the system refuses
/// stops the process; of two first calls at once, the one that loses gives
/// its own back. The system is asked a page more at its own alignment, where
/// its zeroed memory is mapped as it is touched; at a page's, it would be
/// written whole.
#[cfg(not(target_arch = "wasm32"))]
fn mem_at() -> usize {
    use core::sync::atomic::AtomicUsize;
    use core::sync::atomic::Ordering::{AcqRel, Acquire};
    use std::alloc::{GlobalAlloc, Layout, System};
    static BASE: AtomicUsize = AtomicUsize::new(0);
    let base = BASE.load(Acquire);
    if base != 0 {
        return base;
    }
    let Ok(layout) = Layout::from_size_align(mem::SZ as usize + PAGE_BYTES as usize, 16) else {
        std::process::abort();
    };
    // SAFETY: a layout of nonzero size.
    let at = unsafe { System.alloc_zeroed(layout) };
    if at.is_null() {
        std::process::abort();
    }
    let new = page_up(at.expose_provenance());
    match BASE.compare_exchange(0, new, AcqRel, Acquire) {
        Ok(_) => new,
        Err(first) => {
            // SAFETY: what this call reserved, as it reserved it.
            unsafe { System.dealloc(at, layout) };
            first
        }
    }
}

/// Where `.fixed` begins: the page past the module's data.
#[cfg(target_arch = "wasm32")]
pub(crate) fn fixed_at() -> usize {
    unsafe extern "C" {
        /// Where the module's data ends, as its linker places it.
        static __heap_base: u8;
    }
    page_up((&raw const __heap_base).expose_provenance())
}

/// Where `.fixed` begins: the block's base, as nothing else lies in it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn fixed_at() -> usize {
    mem_at()
}

/// The log2 of a page.
const PAGE_LOG2: usize = PAGE_BYTES.trailing_zeros() as usize;

/// 2 GiB, the largest power of two short of the whole address space. The
/// tree starts as one leaf and grows only as requests need it; `end` says
/// how much of the span is real memory.
const SPAN_LOG2: usize = ADDR_BITS - 1;

/// One bit for each possible parent node.
const SPLIT_SZ: usize = (1 << (SPAN_LOG2 - PAGE_LOG2)) / 8;

/// The heap's range: from the page past `.fixed` to the memory's end.
pub(crate) struct Span;

impl Range for Span {
    const MIN_LOG2: usize = PAGE_LOG2;
    const MAX_LOG2: usize = SPAN_LOG2;
    type Split = [u8; SPLIT_SZ];
    const SPLIT_INIT: Self::Split = [0; SPLIT_SZ];

    fn base() -> usize {
        page_up(fixed_at() + mem::FIXED_SZ as usize)
    }

    fn end() -> usize {
        mem_at() + mem::SZ as usize
    }
}

/// The heap, allocated from through `Heap` alone.
static HEAP: HeapMalloc<Span> = HeapMalloc::new();

/// The heap as the core's global allocator, which a terminal crate declares.
pub struct Heap;

// SAFETY: the heap's.
unsafe impl core::alloc::GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        // SAFETY: the caller's.
        unsafe { core::alloc::GlobalAlloc::alloc(&HEAP, layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        // SAFETY: the caller's.
        unsafe { core::alloc::GlobalAlloc::dealloc(&HEAP, ptr, layout) }
    }
}

pub(crate) fn stat() -> HeapStat {
    let s = HEAP.slab();
    HeapStat {
        range: s.range_sz(),
        peak: s.peak(),
        live: s.live(),
        run: s.run_live(),
        slot: s.slot_live(),
        largest_free: s.largest_free(),
        releases: s.releases(),
        refusals: s.refusals(),
    }
}

/// `n` zeroed `T`s, or None when the heap refuses them.
pub(crate) fn zeroed<T: Copy + Default>(n: usize) -> Option<Box<[T]>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).ok()?;
    v.resize(n, T::default());
    Some(v.into_boxed_slice())
}
