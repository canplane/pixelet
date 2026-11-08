// xpute-core/mem/heap.rs

//! The core's heap, whose blocks are named by offset. It is a slab of size
//! classes over a buddy allocator of pages, within the core's `Bounds`. The
//! slab borrows and returns whole runs of pages, so neither level starves
//! while the other has room. `alloc` and `free` serve blocks through
//! whatever `#[global_allocator]` is installed.

use core::marker::PhantomData;

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::slab::SlabMalloc;
use xpute_kit::alloc::ADDR_BITS;

use crate::mem::{at, off_of};

/// Where a core's heap lies: from `base` up to `end`. Each must be a pure
/// function, since the first block is asked for before anything has laid
/// out the memory.
pub trait Bounds {
    fn base() -> usize;
    fn end() -> usize;
}

/// The log2 of a page, the buddy's smallest block.
const PAGE_LOG2: usize = crate::mem::section::PAGE_BYTES.trailing_zeros() as usize;

/// 2 GiB, the largest power of two short of the whole address space. The
/// tree starts as one leaf and grows only as requests need it; `end` says
/// how much of the span is real memory.
const SPAN_LOG2: usize = ADDR_BITS - 1;

/// One bit for each possible parent node. The buddy checks this length on
/// the first request.
const SPLIT_BYTES: usize = (1 << (SPAN_LOG2 - PAGE_LOG2)) / 8;

pub struct Span<B>(PhantomData<B>);

impl<B: Bounds> Range for Span<B> {
    const MIN_LOG2: usize = PAGE_LOG2;
    const MAX_LOG2: usize = SPAN_LOG2;
    type Split = [u8; SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; SPLIT_BYTES];

    fn base() -> usize {
        B::base()
    }
    fn end() -> usize {
        B::end()
    }
}

pub type Heap<B> = SlabMalloc<Span<B>>;

/// One reading of a heap, so that fields compared with each other come from
/// the same moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// Advances each time a page is released, which is when a request of a
    /// page or more that was refused before might now be served. A slot
    /// returned to a run still in use does not advance it
    /// (`SlabMalloc::releases`).
    pub releases: u32,
    pub refusals: u32,
    pub requests: u32,
}

pub fn heap_stat<B: Bounds>(a: &Heap<B>) -> HeapStat {
    HeapStat {
        range: a.range_bytes(),
        peak: a.peak(),
        live: a.live(),
        run: a.run_live(),
        slot: a.slot_live(),
        largest_free: a.largest_free(),
        releases: a.releases(),
        refusals: a.refusals(),
        requests: a.pages().allocs() as u32,
    }
}

/// An offset from the base and a size: freeing needs both.
pub type Block = (usize, u32);

/// The alignment of the widest element a block holds, an `f64` or a `u64`.
const BLOCK_ALIGN: usize = 8;

/// Answers none when there is no room, and for zero bytes.
pub fn alloc(bytes: u32) -> Option<Block> {
    if bytes == 0 {
        return None;
    }
    let layout = std::alloc::Layout::from_size_align(bytes as usize, BLOCK_ALIGN).ok()?;
    // SAFETY: a layout of non-zero size.
    let at = unsafe { std::alloc::alloc(layout) };
    (!at.is_null()).then_some((off_of(at), bytes))
}

/// # Safety
/// `block` must have come from `alloc` and not yet been given back.
pub unsafe fn free((off, bytes): Block) {
    // SAFETY: the caller's; `alloc` made this layout for this address.
    unsafe {
        std::alloc::dealloc(
            at(off),
            std::alloc::Layout::from_size_align_unchecked(bytes as usize, BLOCK_ALIGN),
        )
    }
}
