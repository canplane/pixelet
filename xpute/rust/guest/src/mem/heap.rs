// xpute-guest/mem/heap.rs

//! The heap a guest installs, and blocks of it named by offset. The heap is
//! a slab of size classes over a buddy of pages between the bounds the guest
//! gives (`Bounds`); the slab borrows and returns whole runs of pages, so
//! neither level starves while the other has room. A guest that installs
//! another `#[global_allocator]` still gets its blocks here.

use core::marker::PhantomData;

use xpute_kit::alloc::buddy_tree::Range;
use xpute_kit::alloc::slab::SlabMalloc;

use crate::mem::base::{off_of, ptr_at};

/// Where a guest's heap lies: from `base` to `end`, ascending. A pure
/// function each: the first block is asked for before anything has laid the
/// memory out.
pub trait Bounds {
    fn base() -> usize;
    fn end() -> usize;
}

/// A page, the buddy's leaf.
const PAGE_LOG2: usize = crate::mem::section::PAGE_BYTES.trailing_zeros() as usize;

/// 2 GiB: the largest power of two a 32-bit address space holds. The tree
/// starts as one leaf and grows only as requests need; `end` says what is real.
const SPAN_LOG2: usize = 31;

/// A bit per possible parent node; the compiler cannot derive the length,
/// so the buddy checks it on the first request.
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

/// The heap between `B`'s bounds, for `#[global_allocator]`.
pub type Heap<B> = SlabMalloc<Span<B>>;

/// One reading of a heap, so compared fields are from one moment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStat {
    pub range: usize,
    /// A high-water mark, not a holding.
    pub used: usize,
    pub live: usize,
    pub run: usize,
    pub slot: usize,
    /// Past the tree's current root the range is untouched rather than listed,
    /// so this is not the free space.
    pub largest_free: usize,
    /// Moves exactly when a request refused before might now be served.
    pub releases: u32,
    pub refusals: u32,
    pub requests: u32,
}

pub fn heap_stat<B: Bounds>(a: &Heap<B>) -> HeapStat {
    HeapStat {
        range: a.range_bytes(),
        used: a.used(),
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

/// The widest lane element, `f64` or `u64`.
const BLOCK_ALIGN: usize = 8;

/// None when there is no room, and for zero bytes.
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
/// `block` came from `alloc` and has not been given back.
pub unsafe fn free((at, bytes): Block) {
    // SAFETY: the caller's; `alloc` made this layout for this address.
    unsafe { std::alloc::dealloc(ptr_at(at), std::alloc::Layout::from_size_align_unchecked(bytes as usize, BLOCK_ALIGN)) }
}

/// Freed on drop unless `keep` hands it on.
pub struct Guard(Block);

impl Guard {
    pub fn alloc(bytes: u32) -> Option<Guard> {
        alloc(bytes).map(Guard)
    }

    pub fn keep(self) -> Block {
        let block = self.0;
        core::mem::forget(self);
        block
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: a guard is made only by `Guard::alloc`, and `keep` forgets it.
        unsafe { free(self.0) }
    }
}
