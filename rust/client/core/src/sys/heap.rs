// pxl-core/sys/heap.rs

//! The core's view of its heap, which xpute's guest library owns
//! (xpute-guest heap.rs): the global allocator over the core's memory past
//! `.fixed`, the core's share of it, whose size the core gives where it gives
//! the memory's (the page's worker, pxl-runtime). The memory is never grown,
//! so a full heap stops the core.

use xpute_guest::heap::{self, HEAP};

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

/// Every fixed range begins on a page, as the heap after them does.
pub(crate) const PAGE_BYTES: u32 = heap::PAGE_SZ as u32;

pub(crate) fn stat() -> HeapStat {
    HeapStat {
        range: HEAP.range_sz(),
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
pub(crate) fn zeroed<T: Copy + Default>(n: usize) -> Option<Box<[T]>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).ok()?;
    v.resize(n, T::default());
    Some(v.into_boxed_slice())
}
