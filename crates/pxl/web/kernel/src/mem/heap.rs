// pxl-kernel/mem/heap.rs

//! The heap, as every bracket installs it: a slab of size classes over a
//! buddy of pages, from the page past the last fixed range to the end of the
//! memory. The buddy is what the sections' blocks come from; the slab is what
//! a `Vec` or a `String` comes from, and it borrows and returns whole runs of
//! pages to the same buddy, so neither level can starve while the other has
//! room. What a bracket says is only where its memory lies (`HeapBounds`).

use core::marker::PhantomData;

use xpute_core::alloc::buddy_tree::Range;
use xpute_core::alloc::slab::SlabMalloc;

use crate::host::HeapStat;
use crate::mem::section;

/// Where a bracket's memory lies, as addresses: the heap is what is left of it
/// past the fixed ranges, and what is left is whatever the device gave.
pub trait HeapBounds {
    /// The heap's first byte: `section::heap_at` of the bracket's data end,
    /// from the memory's base. A pure function of the bracket's answer and the
    /// map's constants, with no state and no order to be in: the first block
    /// is asked for long before anything has laid the memory out.
    fn base() -> usize;
    /// One past its last byte.
    fn end() -> usize;
}

/// The largest block the heap's tree can describe. Two gigabytes is what a
/// 32-bit address space holds a whole power of two of, so it is the top of the
/// shape and not a claim about any device: the tree begins as a single leaf and
/// doubles only as far as a request makes it, and how much of it is real comes
/// from `end` at runtime.
const MAX_LOG2: usize = 31;

/// The smallest block, which is the page the rest of the memory is counted in
/// (mem.json's PAGE_BYTES).
const MIN_LOG2: usize = {
    assert!(section::PAGE_BYTES.is_power_of_two());
    section::PAGE_BYTES.trailing_zeros() as usize
};

/// A bit for every node of the tree that can be a parent. The exponents give
/// the length and the compiler will not derive it for us, so it is written
/// once here and `BuddyMalloc` checks it against them on the first request.
const SPLIT_BYTES: usize = (1 << (MAX_LOG2 - MIN_LOG2)) / 8;

/// The heap over a bracket's memory.
pub struct Heap<B>(PhantomData<B>);

impl<B: HeapBounds> Range for Heap<B> {
    const MIN_LOG2: usize = MIN_LOG2;
    const MAX_LOG2: usize = MAX_LOG2;
    type Split = [u8; SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; SPLIT_BYTES];

    fn base() -> usize {
        B::base()
    }
    fn end() -> usize {
        B::end()
    }
}

/// The allocator a bracket installs over its memory.
pub type KernelAlloc<B> = SlabMalloc<Heap<B>>;

/// The allocator's reading, for the ledger and the heap's row.
pub fn heap_stat<B: HeapBounds>(a: &KernelAlloc<B>) -> HeapStat {
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
