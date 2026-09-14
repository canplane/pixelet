//! The mesh section's and the band stores' pools, as the world holds them:
//! a `BuddyAllocator` over a range of the module's memory with `no_spill`
//! — the kernel the client's pool names — with the count of blocks given
//! back beside it, which is what a build refused a block waits on.

use xpute_core::ds::buddy::{BuddyAllocator, BuddyOptions, BuddyStat, PoolKernel};
use xpute_core::kit::array::ArrayBuffer;

pub struct Pool {
    inner: BuddyAllocator,
    /// Blocks given back: the count changes when, and only when, a request
    /// refused before might be served.
    pub releases: u32,
    /// The kernel's stat, in BuddyStat's first twelve fields' order, as of
    /// the last `stat_words`: a row the glue reads by address.
    words: [u32; 12],
}

impl Pool {
    /// A pool over `bytes` bytes at `start` of the memory of `mem_bytes`
    /// at `mem`, cut in regions of `region_bytes` with blocks of at least
    /// `min_block_bytes`. The range must be whole regions.
    ///
    /// # Safety
    /// `mem` must address `mem_bytes` bytes that stay valid for the pool's
    /// life, and nothing else may write a free block's first two words.
    pub unsafe fn new(mem: *mut u8, mem_bytes: usize, start: usize, bytes: usize, region_bytes: u32, min_block_bytes: u32) -> Pool {
        let backing = ArrayBuffer::from_raw(mem, mem_bytes);
        let inner = BuddyAllocator::new(BuddyOptions {
            min_block_bytes: Some(min_block_bytes),
            region_bytes: Some(region_bytes),
            max_idle_regions: Some(0),
            backing: Some(backing),
            backing_offset: Some(start as u32),
            backing_bytes: Some(bytes as u32),
            no_spill: Some(true),
            ..Default::default()
        })
        .unwrap_or_else(|e| panic!("{e}"));
        Pool { inner, releases: 0, words: [0; 12] }
    }

    /// Borrows at least `bytes`: the block's address, or none when the range
    /// is used up or the request is larger than a region.
    pub fn acquire(&mut self, bytes: u32) -> Option<usize> {
        if bytes == 0 {
            return None;
        }
        self.inner.try_acquire(bytes).map(|b| b.byte_offset as usize)
    }

    /// Returns the block of `bytes` at `at`. False for a block the pool
    /// does not hold or holds already.
    pub fn release(&mut self, at: usize, bytes: u32) -> bool {
        let ok = PoolKernel::release(&mut self.inner, at as u32, bytes);
        if ok {
            self.releases += 1;
        }
        ok
    }

    pub fn is_free(&self, at: usize, bytes: u32) -> bool {
        PoolKernel::is_free(&self.inner, at as u32, bytes)
    }

    pub fn stat(&self) -> BuddyStat {
        self.inner.stat()
    }

    /// The stat as the kernel reports it, at a fixed address.
    pub fn stat_words(&mut self) -> *const u32 {
        self.words = PoolKernel::stat(&self.inner);
        self.words.as_ptr()
    }

    /// The bytes of the range the pool cuts.
    pub fn range_bytes(&self) -> usize {
        self.inner.stat().backing_bytes as usize
    }
}
