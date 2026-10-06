// pxl-kernel/mem/heap.rs

//! The heap every bracket installs: a slab of size classes over a buddy of
//! pages, from past the last fixed range to the end of memory. The slab
//! borrows and returns whole runs of pages, so neither level starves while
//! the other has room.

use core::marker::PhantomData;

use xpute_core::alloc::buddy_tree::Range;
use xpute_core::alloc::slab::SlabMalloc;

use crate::door::host::HeapStat;
use crate::mem::section;

pub trait HeapBounds {
    /// Must be a pure function: the first block is asked for before anything
    /// has laid the memory out.
    fn base() -> usize;
    fn end() -> usize;
}

/// 2 GiB: the largest power of two a 32-bit address space holds. The tree
/// starts as one leaf and grows only as requests need; `end` says what is real.
const MAX_LOG2: usize = 31;

const MIN_LOG2: usize = {
    assert!(section::PAGE_BYTES.is_power_of_two());
    section::PAGE_BYTES.trailing_zeros() as usize
};

/// A bit per possible parent node; the compiler cannot derive the length,
/// so `BuddyMalloc` checks it on the first request.
const SPLIT_BYTES: usize = (1 << (MAX_LOG2 - MIN_LOG2)) / 8;

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

pub type KernelAlloc<B> = SlabMalloc<Heap<B>>;

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
