//! A buddy pool over a range of the linear memory: @xpute/core/ds/buddy.ts's
//! backed side, in Rust.
//!
//! Regions of one size are cut from the range front to back and never given
//! back; inside a region a request rounds up to a power-of-two order, a free
//! block of a larger order splits in half repeatedly to serve it, and a
//! released block merges back with its buddy whenever that buddy is free.
//! The free blocks of each order are a doubly linked list threaded through
//! the free blocks themselves — the first two words of a free block hold the
//! region-relative offsets of the next and previous free block of its order
//! — so the pool allocates nothing of its own per block; only the per-region
//! "is this block free" bitmaps live on the heap. Why buddy and not plain
//! per-class lists, and why the links live in the blocks, is in the
//! TypeScript, which this mirrors so that the same sequence of requests
//! yields the same addresses (the equivalence test in buddy.test.ts).
//!
//! What the TypeScript pool does past the range — plain buffers of its own,
//! spills, evictions — stays on that side: the module can reach only the
//! memory it was given, and the range is what it owns.

/// Marks a list's end and a block's missing neighbour.
const NIL: u32 = 0xffff_ffff;

/// What the pool has done, in the TypeScript stat's terms.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PoolStat {
    /// Bytes in blocks currently borrowed, as requested before rounding up.
    pub live_bytes: u32,
    /// Region bytes not currently carved into a live block.
    pub idle_bytes: u32,
    /// Bytes across the regions cut so far.
    pub reserved_bytes: u32,
    pub regions: u32,
    pub live_blocks: u32,
    /// Acquires served from the existing regions.
    pub hits: u32,
    /// Acquires that had to cut a new region.
    pub misses: u32,
    pub splits: u32,
    pub merges: u32,
    pub max_block_bytes: u32,
    /// Bytes of the range cut into regions so far.
    pub backing_used: u32,
    /// Acquires refused: the range used up, or a request past a region.
    pub refusals: u32,
}

struct Region {
    /// Where the region starts, as an offset from the pool's memory base.
    base: usize,
    /// Offset of the first free block of each order, or NIL.
    head: Vec<u32>,
    /// Per order, one byte per block of that order in the region.
    is_free: Vec<Vec<u8>>,
    /// Bytes carved into live blocks.
    used_bytes: u32,
}

impl Region {
    fn new(base: usize, max_order: u32) -> Region {
        let is_free = (0..=max_order).map(|k| vec![0u8; 1 << (max_order - k)]).collect();
        Region {
            base,
            head: vec![NIL; max_order as usize + 1],
            is_free,
            used_bytes: 0,
        }
    }
}

/// One pool over `[start, start + bytes)` of a memory whose base is `mem`.
pub struct Pool {
    mem: *mut u8,
    min_shift: u32,
    max_order: u32,
    region_bytes: u32,
    start: usize,
    end: usize,
    /// Where the next region is cut.
    cut: usize,
    regions: Vec<Region>,
    pub stat: PoolStat,
}

fn shift_of(bytes: u32) -> u32 {
    32 - (bytes - 1).leading_zeros()
}

impl Pool {
    /// A pool over `bytes` bytes at `start` of the memory at `mem`, cut into
    /// regions of `region_bytes` (rounded up to a power of two) with blocks
    /// no smaller than `min_block_bytes` (likewise, at least 8). The range
    /// must be whole regions.
    ///
    /// # Safety
    /// `mem` must address at least `start + bytes` bytes that stay valid
    /// for the pool's life, and nothing else may write a free block's first
    /// two words.
    pub unsafe fn new(mem: *mut u8, start: usize, bytes: usize, region_bytes: u32, min_block_bytes: u32) -> Pool {
        assert!(min_block_bytes >= 8 && region_bytes >= min_block_bytes, "pool: bad region shape");
        let min_shift = shift_of(min_block_bytes);
        let max_order = shift_of(region_bytes) - min_shift;
        let region_bytes = 1u32 << (min_shift + max_order);
        assert!(
            start.is_multiple_of(region_bytes as usize) && bytes.is_multiple_of(region_bytes as usize),
            "pool: range is not whole regions"
        );
        Pool {
            mem,
            min_shift,
            max_order,
            region_bytes,
            start,
            end: start + bytes,
            cut: start,
            regions: Vec::new(),
            stat: PoolStat::default(),
        }
    }

    /// The bytes of the range the pool cuts.
    pub fn range_bytes(&self) -> usize {
        self.end - self.start
    }

    pub fn region_bytes(&self) -> u32 {
        self.region_bytes
    }

    fn order_of(&self, bytes: u32) -> u32 {
        shift_of(bytes).max(self.min_shift) - self.min_shift
    }

    fn word(&self, at: usize) -> *mut u32 {
        self.mem.wrapping_add(at) as *mut u32
    }

    fn mark_free(&mut self, r: usize, order: u32, offset: u32) {
        let base = self.regions[r].base;
        let next = self.regions[r].head[order as usize];
        // SAFETY: a free block's first two words are the pool's (new).
        unsafe {
            *self.word(base + offset as usize) = next;
            *self.word(base + offset as usize + 4) = NIL;
            if next != NIL {
                *self.word(base + next as usize + 4) = offset;
            }
        }
        let region = &mut self.regions[r];
        region.head[order as usize] = offset;
        region.is_free[order as usize][(offset >> (self.min_shift + order)) as usize] = 1;
    }

    fn mark_used(&mut self, r: usize, order: u32, offset: u32) {
        let base = self.regions[r].base;
        // SAFETY: as mark_free.
        let (next, prev) = unsafe { (*self.word(base + offset as usize), *self.word(base + offset as usize + 4)) };
        if prev == NIL {
            self.regions[r].head[order as usize] = next;
        } else {
            unsafe { *self.word(base + prev as usize) = next };
        }
        if next != NIL {
            unsafe { *self.word(base + next as usize + 4) = prev };
        }
        self.regions[r].is_free[order as usize][(offset >> (self.min_shift + order)) as usize] = 0;
    }

    /// Offset of a free block of exactly `order` in region `r`, splitting a
    /// larger one if needed.
    fn alloc_in(&mut self, r: usize, order: u32) -> Option<u32> {
        let mut k = order;
        while k <= self.max_order && self.regions[r].head[k as usize] == NIL {
            k += 1;
        }
        if k > self.max_order {
            return None;
        }
        let offset = self.regions[r].head[k as usize];
        self.mark_used(r, k, offset);
        while k > order {
            k -= 1;
            self.stat.splits += 1;
            self.mark_free(r, k, offset + (1 << (self.min_shift + k)));
        }
        Some(offset)
    }

    /// Borrows at least `bytes`: the address of the block, or None when the
    /// range is used up or the request is larger than a region. Contents
    /// are undefined.
    pub fn acquire(&mut self, bytes: u32) -> Option<usize> {
        if bytes == 0 {
            return None;
        }
        if bytes > self.stat.max_block_bytes {
            self.stat.max_block_bytes = bytes;
        }
        if bytes > self.region_bytes {
            self.stat.refusals += 1;
            return None;
        }
        let order = self.order_of(bytes);
        let block_bytes = 1u32 << (self.min_shift + order);
        for r in 0..self.regions.len() {
            if let Some(offset) = self.alloc_in(r, order) {
                self.regions[r].used_bytes += block_bytes;
                self.stat.hits += 1;
                self.stat.live_blocks += 1;
                self.stat.live_bytes += bytes;
                self.stat.idle_bytes -= block_bytes;
                return Some(self.regions[r].base + offset as usize);
            }
        }
        if self.cut + self.region_bytes as usize > self.end {
            self.stat.refusals += 1;
            return None;
        }
        let r = self.regions.len();
        self.regions.push(Region::new(self.cut, self.max_order));
        self.cut += self.region_bytes as usize;
        self.mark_free(r, self.max_order, 0);
        self.stat.backing_used = (self.cut - self.start) as u32;
        self.stat.regions += 1;
        self.stat.reserved_bytes += self.region_bytes;
        self.stat.idle_bytes += self.region_bytes;
        let offset = self.alloc_in(r, order).expect("a fresh region serves any order");
        self.regions[r].used_bytes += block_bytes;
        self.stat.misses += 1;
        self.stat.live_blocks += 1;
        self.stat.live_bytes += bytes;
        self.stat.idle_bytes -= block_bytes;
        Some(self.regions[r].base + offset as usize)
    }

    fn locate(&self, at: usize) -> Option<usize> {
        if at < self.start || at >= self.cut {
            return None;
        }
        Some((at - self.start) / self.region_bytes as usize)
    }

    /// Whether the block of `bytes` at `at` is free — on a free list itself,
    /// or inside a larger free block it merged into — so a release of it
    /// would be a double release.
    pub fn is_free(&self, at: usize, bytes: u32) -> bool {
        let Some(r) = self.locate(at) else { return false };
        let offset = (at - self.regions[r].base) as u32;
        (self.order_of(bytes)..=self.max_order).any(|k| self.regions[r].is_free[k as usize][(offset >> (self.min_shift + k)) as usize] == 1)
    }

    /// Returns the block of `bytes` at `at`, merging it upward as far as its
    /// buddies allow. False when the address is outside the pool's regions
    /// or the block is already free.
    pub fn release(&mut self, at: usize, bytes: u32) -> bool {
        let Some(r) = self.locate(at) else { return false };
        if self.is_free(at, bytes) {
            return false;
        }
        let order = self.order_of(bytes);
        let offset = (at - self.regions[r].base) as u32;
        self.stat.live_blocks -= 1;
        self.stat.live_bytes -= bytes;
        let block_bytes = 1u32 << (self.min_shift + order);
        let mut k = order;
        let mut at = offset;
        while k < self.max_order {
            let buddy = at ^ (1 << (self.min_shift + k));
            if self.regions[r].is_free[k as usize][(buddy >> (self.min_shift + k)) as usize] != 1 {
                break;
            }
            self.mark_used(r, k, buddy);
            self.stat.merges += 1;
            at = at.min(buddy);
            k += 1;
        }
        self.mark_free(r, k, at);
        self.regions[r].used_bytes -= block_bytes;
        self.stat.idle_bytes += block_bytes;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGION: u32 = 1 << 16;

    fn pool(regions: usize) -> (Vec<u8>, Pool) {
        let mut mem = vec![0u8; REGION as usize * regions];
        let p = unsafe { Pool::new(mem.as_mut_ptr(), 0, mem.len(), REGION, 1 << 12) };
        (mem, p)
    }

    #[test]
    fn a_released_block_is_handed_to_the_next_request_of_its_order() {
        let (_m, mut p) = pool(1);
        let a = p.acquire(4096).unwrap();
        let _b = p.acquire(4096).unwrap();
        assert!(p.release(a, 4096));
        assert_eq!(p.acquire(4096), Some(a));
    }

    #[test]
    fn blocks_never_overlap_while_live() {
        let (_m, mut p) = pool(8);
        let mut got = Vec::new();
        for i in 0..20u32 {
            let bytes = 4096 << (i % 4);
            let at = p.acquire(bytes).unwrap();
            for &(o, b) in &got {
                assert!(at + bytes as usize <= o || o + b as usize <= at, "overlap");
            }
            got.push((at, bytes));
        }
    }

    #[test]
    fn a_large_block_splits_and_merges_back() {
        let (_m, mut p) = pool(1);
        let whole = p.acquire(REGION).unwrap();
        assert!(p.release(whole, REGION));
        let a = p.acquire(4096).unwrap();
        let b = p.acquire(4096).unwrap();
        assert!(p.stat.splits > 0);
        assert!(p.release(a, 4096) && p.release(b, 4096));
        assert!(p.stat.merges >= p.stat.splits);
        assert_eq!(p.acquire(REGION), Some(whole));
    }

    #[test]
    fn regions_come_out_of_the_range_front_to_back_and_refuse_past_it() {
        let (_m, mut p) = pool(2);
        assert_eq!(p.acquire(REGION), Some(0));
        assert_eq!(p.acquire(REGION), Some(REGION as usize));
        assert_eq!(p.acquire(REGION), None);
        assert_eq!(p.stat.refusals, 1);
        assert_eq!(p.acquire(REGION * 2), None, "larger than a region");
        assert_eq!(p.stat.backing_used, REGION * 2);
    }

    #[test]
    fn a_block_past_the_first_region_round_trips() {
        let (_m, mut p) = pool(2);
        let _first = p.acquire(REGION).unwrap();
        let small = p.acquire(4096).unwrap();
        assert!(small >= REGION as usize);
        assert!(p.release(small, 4096));
        assert_eq!(p.acquire(4096), Some(small));
    }

    #[test]
    fn a_double_release_is_refused() {
        let (_m, mut p) = pool(1);
        let a = p.acquire(4096).unwrap();
        assert!(p.release(a, 4096));
        assert!(!p.release(a, 4096));
        assert!(!p.release(REGION as usize * 3, 4096), "outside every region");
        assert_eq!(p.stat.live_blocks, 0);
    }
}
