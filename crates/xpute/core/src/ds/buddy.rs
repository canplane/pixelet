// xpute_core/ds/buddy.rs

//! Buddy allocator for buffers whose deaths are unordered.
//!
//! The sibling of arena.rs, for the lifetime arena.rs structurally cannot
//! serve. Arena/Vector are bump allocators: everything they hand out dies at
//! one rewind, so they fit "build it, consume it, drop it all" exactly. A
//! retained buffer — one an owner holds for as long as *it* lives, released
//! whenever that owner happens to die — has no such common rewind point, and
//! a bump cursor can never reclaim the middle of its own range.
//!
//! Memory comes from a list of fixed-size **regions**, each one ArrayBuffer
//! carved by the classic buddy scheme: a request rounds up to a power-of-two
//! order, a free block of a larger order splits in half repeatedly to serve
//! it, and a released block merges back with its buddy whenever that buddy is
//! also free. Allocation and release are both O(orders).
//!
//! **Why buddy and not plain per-class free lists.** Plain free lists were
//! tried first and measured: a 17-36% hit rate with the lists sitting full of
//! blocks exactly one order above every request. The reason is structural
//! rather than incidental — the workload migrates between classes by
//! definition. A renderer dropping a chunk's fine LOD tier and asking for a
//! coarse one in the same breath frees a large block and requests a small
//! one; a zoom back in does the reverse. Per-class lists cannot move bytes
//! between classes at all, so every such transition is a miss on one side and
//! dead inventory on the other. Splitting fixes one direction, merging fixes
//! the other, and needing both is what makes this a buddy allocator rather
//! than a pool with a hack in it.
//!
//! Byte blocks, not typed arrays, so one logically-single buffer with several
//! differently-typed spans (a mesh's position/normal/index/...) is one
//! allocation rather than one per span. Callers carve their own views from
//! `buf` at `byte_offset`; the minimum order is a multiple of 8 bytes and
//! every order is a power-of-two multiple of it, so any view alignment up to
//! f64 is satisfied without the caller thinking about it.
//!
//! Deliberately portable. Here each region is a `Box<[u8]>` (or one big
//! range of the linear memory under WASM) and everything above is index
//! arithmetic that translates line for line. Under WASM, where linear memory
//! grows and never shrinks, buddy-over-one-region is the canonical shape
//! rather than a nicety, and `max_idle_regions` is the knob that decides
//! whether a peak becomes permanent residency.
//!
//! The one failure mode this introduces is a view outliving its release:
//! another borrower gets those exact bytes, so it is silent corruption rather
//! than a stale read. `poison` makes it loud, and the double-release guard is
//! always on.

#![allow(non_snake_case)]

use std::collections::{HashMap, HashSet};

use crate::kit::array::{ArrayBuffer, TypedArray};
use crate::sys::console;
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

#[derive(Default)]
pub struct BuddyOptions {
    /// Smallest allocation, and the alignment every block satisfies.
    pub min_block_bytes: Option<u32>,
    /// One region's size. A request larger than this spills to its own ArrayBuffer.
    pub region_bytes: Option<u32>,
    /// How many fully-empty regions to keep rather than release to the
    /// collector. Borrowed bytes are not a surplus to reclaim — refusing to
    /// retain them frees nothing and only turns every release into a free — so
    /// what a ceiling can honestly bound is the idle surplus, and with buddy
    /// that surplus is countable in whole empty regions.
    pub max_idle_regions: Option<u32>,
    /// Fill released blocks with NaN so a stale reader is loud. Dev only — it is a memset per release.
    pub poison: Option<bool>,
    /// Hand every request its own buffer and reuse nothing, so a caller whose
    /// ownership is not yet airtight cannot corrupt one borrower with another's
    /// writes. The allocator becomes a plain `new ArrayBuffer` with bookkeeping
    /// — every acquire counts as a spill — which is the *previous* behaviour of
    /// whatever it replaced, and therefore always a safe place to stand.
    ///
    /// A switch worth having: pooling converts a use-after-free from a harmless
    /// stale read into silent cross-borrower corruption, so "is the pool at
    /// fault" has to be answerable by flipping one flag rather than by reverting
    /// a change.
    pub bypass: Option<bool>,
    /// A buffer to carve regions out of before allocating any of their own —
    /// typically a fixed-length `WebAssembly.Memory`'s buffer, so JS and a
    /// future WASM side address the same bytes.
    ///
    /// Regions are cut from it front to back, one `region_bytes` at a time, and
    /// are never handed back: the bytes belong to the backing, not to the
    /// collector, so releasing them would free nothing. They sit idle and are
    /// reused instead, and `max_idle_regions` governs only the plain regions
    /// allocated once the backing is used up. The backing must never be
    /// resized — a view into a detached buffer reads as length zero.
    pub backing: Option<ArrayBuffer>,
    /// The range of the backing this pool may carve: a start offset, which a
    /// block must be able to land on (`min_block_bytes`), and a byte length,
    /// whose tail past the last whole region is never cut. Several pools over one memory
    /// take disjoint ranges — the sections of a core memory — and none can
    /// reach another's. Defaults to the whole backing.
    pub backing_offset: Option<u32>,
    pub backing_bytes: Option<u32>,
    /// Refuse rather than spill: once the backing range is used up, `acquire`
    /// returns null instead of opening a plain buffer, and a request larger
    /// than a region returns null too. For a pool whose bytes a kernel over the
    /// backing must be able to see — nothing it lends may live outside it.
    pub no_spill: Option<bool>,
    /// A kernel that owns the backing range: the buddy over it runs there,
    /// and this pool only names its blocks — an address in, an address out —
    /// and keeps what the kernel cannot do, the plain regions past the range.
    /// Given with `backing`, and the range is the kernel's to cut.
    pub kernel: Option<Box<dyn PoolKernel>>,
}

/// The backed side of a pool, run elsewhere (on the TypeScript side, this
/// crate through pxl-wasm's pool exports; here, another pool over the same
/// backing): blocks named by their address in the backing. `acquire`
/// answers 0 for a refusal; `release` false for a block the kernel does not
/// hold or holds already. `stat` is the kernel's count of what it did, in
/// BuddyStat's first twelve fields' order.
pub trait PoolKernel {
    fn acquire(&mut self, bytes: u32) -> u32;
    fn release(&mut self, at: u32, bytes: u32) -> bool;
    fn is_free(&self, at: u32, bytes: u32) -> bool;
    fn stat(&self) -> [u32; 12];
}

/// The `region` of a block the kernel lent: it is in the backing, and the
/// kernel is the one to ask about it.
const KERNEL_REGION: i32 = -2;

/// The handle of a kernel block: its address, and the high bit to say so
/// (no backing is 2 GB). A block in a plain region keeps the packed form.
const KERNEL_HANDLE: u32 = 0x8000_0000;

/// One borrowed span: `bytes` of `buf` starting at `byte_offset`. The region
/// behind `buf` is shared with other live blocks, so a view must always be
/// built from `byte_offset` with an explicit length, never from `buf` alone.
#[derive(Clone)]
pub struct Block {
    pub buf: ArrayBuffer,
    pub byte_offset: u32,
    pub bytes: u32,
    /// Buddy order, or -1 for a spill/empty block that owns its whole buffer.
    pub order: i32,
    /// Which region `buf` belongs to; -1 alongside order -1.
    pub region: i32,
    /// Guard state, not a capability: a released block's views stay reachable.
    pub free: bool,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct BuddyStat {
    /// Bytes in blocks currently borrowed (as requested, before rounding up).
    pub live_bytes: u32,
    /// Region bytes not currently carved into a live block.
    pub idle_bytes: u32,
    /// Total bytes across all regions.
    pub reserved_bytes: u32,
    pub regions: u32,
    pub live_blocks: u32,
    /// Acquires served from the existing regions.
    pub hits: u32,
    /// Acquires that had to open a new region.
    pub misses: u32,
    /// Acquires larger than one region, given their own buffer and never reused.
    pub spills: u32,
    /// Empty regions handed back to the collector.
    pub evictions: u32,
    /// Splits and merges performed — the work buddy does that free lists cannot.
    pub splits: u32,
    /// How many merges happened; a healthy steady state keeps this near `splits`.
    pub merges: u32,
    /// Largest single request seen. What region_bytes has to be chosen against:
    /// below it, every such request spills and is never reused.
    pub max_block_bytes: u32,
    /// Bytes of the backing range cut into regions so far, and the range's
    /// size — how close the pool is to its first spill or refusal.
    pub backing_used: u32,
    pub backing_bytes: u32,
    /// Acquires refused under `no_spill`.
    pub refusals: u32,
}

thread_local! {
    static DETACHED_BUF: ArrayBuffer = ArrayBuffer::new(0);
}

/// The zero-size block, and equally the stand-in for a buffer set built
/// outside any pool (a test fixture, a slice taken for inspection): it owns
/// nothing, so releasing it is a no-op rather than an error. One buffer
/// stands behind every copy, so `is_detached` is the identity test the
/// TypeScript's `===` is.
pub fn DETACHED_BLOCK() -> Block {
    Block {
        buf: DETACHED_BUF.with(|b| b.clone()),
        byte_offset: 0,
        bytes: 0,
        order: -1,
        region: -1,
        free: false,
    }
}

impl Block {
    pub fn is_detached(&self) -> bool {
        DETACHED_BUF.with(|b| self.buf.is(b))
    }
}

fn shift_of(bytes: u32) -> u32 {
    32 - (bytes.wrapping_sub(1)).leading_zeros()
}

/// One buddy-carved ArrayBuffer.
///
/// The free blocks of each order are a doubly linked list threaded through
/// the free blocks themselves — the first two words of a free block hold the
/// offsets of the next and previous free block of its order — which is the
/// classic shape and the one that costs nothing: no container per order, no
/// iterator to take the head, no allocation anywhere in the allocator. A
/// block is only written to while it is free, and a borrower gets its bytes
/// dirty in any case. `is_free` answers "is the block of order k at this
/// offset free" in O(1) — the buddy test on release needs exactly that
/// question, and it cannot be read off the list without walking it. It is a
/// bit a block and not a byte: a byte costs the region's bytes over the
/// minimum block, which is a fiftieth of the region at a page and an eighth of
/// it at sixteen bytes — the size a global allocator's smallest class needs.
///
/// Offsets on the lists are region-relative and word-aligned by
/// construction (the minimum order is at least eight bytes); NIL marks a
/// list's end and a block's missing neighbour.
const NIL: u32 = 0xffffffff;

struct Region {
    buf: ArrayBuffer,
    /// The region's bytes as words, for the list links inside free blocks.
    words: TypedArray<u32>,
    /// Offset of the first free block of each order, or NIL.
    head: Vec<u32>,
    /// A bit per block per order, order 0 first: order k's bits start at
    /// `bit_at[k]` and there are `1 << (max_order - k)` of them.
    is_free: Vec<u8>,
    bit_at: Vec<u32>,
    /// Bytes carved into live blocks; a region with 0 is fully reclaimable.
    used_bytes: u32,
    min_shift: u32,
    max_order: u32,
    /// Where this region starts inside `buf`: zero for a region that owns
    /// its buffer, and some multiple of the region size for one carved from
    /// a shared backing. Offsets inside the region — the ones the buddy
    /// arithmetic works in — are relative to it; a block's `byte_offset` is
    /// absolute, so it can index `buf` directly.
    base: u32,
}

struct Counts {
    splits: u32,
    merges: u32,
}

impl Region {
    fn new(min_shift: u32, max_order: u32, backing: Option<ArrayBuffer>, base: u32) -> Region {
        let bytes = 1u32 << (min_shift + max_order);
        let buf = backing.unwrap_or_else(|| ArrayBuffer::new(bytes as usize));
        let words = TypedArray::<u32>::over(buf.clone(), base as usize, (bytes >> 2) as usize);
        let mut bit_at = Vec::with_capacity(max_order as usize + 1);
        let mut bits = 0u32;
        for k in 0..=max_order {
            bit_at.push(bits);
            bits += 1 << (max_order - k);
        }
        let mut region = Region {
            buf,
            words,
            head: vec![NIL; max_order as usize + 1],
            is_free: vec![0u8; bits.div_ceil(8) as usize],
            bit_at,
            used_bytes: 0,
            min_shift,
            max_order,
            base,
        };
        region.mark_free(max_order, 0);
        region
    }

    fn mark_free(&mut self, order: u32, offset: u32) {
        let words = &self.words;
        let w = (offset >> 2) as usize;
        let next = self.head[order as usize];
        words.put(w, next);
        words.put(w + 1, NIL);
        if next != NIL {
            words.put((next >> 2) as usize + 1, offset);
        }
        self.head[order as usize] = offset;
        self.set_free(order, offset, true);
    }

    fn mark_used(&mut self, order: u32, offset: u32) {
        let words = &self.words;
        let w = (offset >> 2) as usize;
        let next = words.get(w);
        let prev = words.get(w + 1);
        if prev == NIL {
            self.head[order as usize] = next;
        } else {
            words.put((prev >> 2) as usize, next);
        }
        if next != NIL {
            words.put((next >> 2) as usize + 1, prev);
        }
        self.set_free(order, offset, false);
    }

    /// Offset of a free block of exactly `order`, splitting a larger one if
    /// needed; -1 when the region cannot serve it. Returns splits performed via
    /// `counts`, which the pool folds into its own stats.
    fn alloc(&mut self, order: u32, counts: &mut Counts) -> i64 {
        let mut k = order;
        while k <= self.max_order && self.head[k as usize] == NIL {
            k += 1;
        }
        if k > self.max_order {
            return -1;
        }

        let offset = self.head[k as usize];
        self.mark_used(k, offset);
        while k > order {
            k -= 1;
            counts.splits += 1;
            self.mark_free(k, offset + (1 << (self.min_shift + k)));
        }
        offset as i64
    }

    /// Which bit says the block of `order` at `offset` is free: the byte it
    /// lies in, and its mask.
    fn bit(&self, order: u32, offset: u32) -> (usize, u8) {
        let i = self.bit_at[order as usize] + (offset >> (self.min_shift + order));
        ((i >> 3) as usize, 1 << (i & 7))
    }

    fn set_free(&mut self, order: u32, offset: u32, on: bool) {
        let (byte, mask) = self.bit(order, offset);
        if on {
            self.is_free[byte] |= mask;
        } else {
            self.is_free[byte] &= !mask;
        }
    }

    /// Whether the block of `order` at `offset` is itself on a free list.
    fn is_block_free(&self, order: u32, offset: u32) -> bool {
        let (byte, mask) = self.bit(order, offset);
        self.is_free[byte] & mask != 0
    }

    /// Returns a block and merges it upward as far as its buddies allow.
    fn release(&mut self, order: u32, offset: u32, counts: &mut Counts) {
        let mut k = order;
        let mut at = offset;
        while k < self.max_order {
            let buddy = at ^ (1 << (self.min_shift + k));
            if !self.is_block_free(k, buddy) {
                break;
            }
            self.mark_used(k, buddy);
            counts.merges += 1;
            at = at.min(buddy);
            k += 1;
        }
        self.mark_free(k, at);
    }
}

pub struct BuddyAllocator {
    pub min_block_bytes: u32,
    pub region_bytes: u32,
    pub max_idle_regions: u32,

    /// The pool's own counts: with a kernel, the plain side only — `stat`
    /// folds the kernel's in.
    _stat: BuddyStat,
    /// The split and merge counts the regions fold into, kept beside
    /// `_stat` so a region and the stat are borrowed apart.
    _counts: Counts,

    _min_shift: u32,
    _max_order: u32,
    _regions: Vec<Option<Region>>,
    _poison: bool,
    _bypass: bool,
    _no_spill: bool,
    _backing: Option<ArrayBuffer>,
    _kernel: Option<Box<dyn PoolKernel>>,
    /// The backing range: where it starts, and how far into it regions have
    /// been cut. `_backing_used` is an absolute offset.
    _backing_offset: u32,
    _backing_end: u32,
    _backing_used: u32,
    /// Indices of regions carved from the backing — never reclaimed.
    _backed: HashSet<usize>,
    /// Region index by its own buffer — how a plain region's address finds it.
    /// A backed region needs none: `_locate` reads its index off the address.
    _plain_at: HashMap<usize, usize>,
    /// Buffers handed out whole — a spill is released by address too.
    _spills: HashSet<usize>,
    /// Bits a handle spends on the order.
    _order_bits: u32,
}

impl BuddyAllocator {
    pub fn new(opts: BuddyOptions) -> Result<BuddyAllocator, MarshalError> {
        let min_block_bytes = opts.min_block_bytes.unwrap_or(1 << 12);
        let region_bytes = opts.region_bytes.unwrap_or(1 << 26);
        let max_idle_regions = opts.max_idle_regions.unwrap_or(2);
        let poison = opts.poison.unwrap_or(false);
        let bypass = opts.bypass.unwrap_or(false);
        let backing = opts.backing;
        let no_spill = opts.no_spill.unwrap_or(false);
        let kernel = opts.kernel;
        if min_block_bytes < 8 || region_bytes < min_block_bytes {
            return Err(MarshalError::new(
                Errno::EINVAL,
                Some(&format!("pool: bad region shape min={min_block_bytes} region={region_bytes}")),
                None,
            ));
        }
        let _min_shift = shift_of(min_block_bytes);
        let min_block_bytes = 1 << _min_shift;
        let _max_order = shift_of(region_bytes) - _min_shift;
        let region_bytes = 1 << (_min_shift + _max_order);
        if kernel.is_some() && backing.is_none() {
            return Err(MarshalError::new(Errno::EINVAL, Some("pool: a kernel needs the backing it runs over"), None));
        }
        // The range starts where a block can and runs as far as it is given;
        // the tail past its last whole region is never cut, which is what the
        // default takes as the length. A start that is not a whole number of
        // regions is fine — `_locate` reads a region's index off the address
        // relative to the start, so a caller cutting sections out of one memory
        // can end each where its content does.
        let offset = opts.backing_offset.unwrap_or(0);
        let whole = match &backing {
            None => 0,
            Some(b) => {
                let past = (b.byte_length() as i64 - offset as i64).max(0) as u32;
                past - past % region_bytes
            }
        };
        let bytes = opts.backing_bytes.unwrap_or(whole);
        if let Some(b) = &backing {
            if !offset.is_multiple_of(min_block_bytes) || (offset as usize + bytes as usize) > b.byte_length() {
                return Err(MarshalError::new(
                    Errno::EINVAL,
                    Some(&format!("pool: backing range {offset}+{bytes} is not a block-aligned range inside {}", b.byte_length())),
                    None,
                ));
            }
        }
        if no_spill && backing.is_none() {
            return Err(MarshalError::new(Errno::EINVAL, Some("pool: no_spill needs a backing to refuse past"), None));
        }
        let _stat = BuddyStat {
            backing_bytes: if backing.is_some() { bytes } else { 0 },
            ..BuddyStat::default()
        };
        Ok(BuddyAllocator {
            min_block_bytes,
            region_bytes,
            max_idle_regions,
            _stat,
            _counts: Counts { splits: 0, merges: 0 },
            _min_shift,
            _max_order,
            _regions: Vec::new(),
            _poison: poison,
            _bypass: bypass,
            _no_spill: no_spill,
            _backing_offset: offset,
            _backing_used: offset,
            _backing_end: if backing.is_some() { offset + bytes } else { 0 },
            _backing: backing,
            _kernel: kernel,
            _backed: HashSet::new(),
            _plain_at: HashMap::new(),
            _spills: HashSet::new(),
            _order_bits: shift_of(_max_order + 2),
        })
    }

    /// What the pool has done, both sides together when a kernel holds the
    /// backed one. Read fresh each time.
    pub fn stat(&self) -> BuddyStat {
        let mut own = self._stat;
        own.splits = self._counts.splits;
        own.merges = self._counts.merges;
        let Some(k) = &self._kernel else { return own };
        let ks = k.stat();
        BuddyStat {
            live_bytes: own.live_bytes + ks[0],
            idle_bytes: own.idle_bytes + ks[1],
            reserved_bytes: own.reserved_bytes + ks[2],
            regions: own.regions + ks[3],
            live_blocks: own.live_blocks + ks[4],
            hits: own.hits + ks[5],
            misses: own.misses + ks[6],
            spills: own.spills,
            evictions: own.evictions,
            splits: own.splits + ks[7],
            merges: own.merges + ks[8],
            max_block_bytes: own.max_block_bytes.max(ks[9]),
            backing_used: ks[10],
            backing_bytes: own.backing_bytes,
            // The kernel refuses past its range whatever the pool's policy; for a
            // pool that may spill, that is where a plain region begins, not a
            // refusal the caller saw.
            refusals: own.refusals + if self._no_spill { ks[11] } else { 0 },
        }
    }

    /// Borrows at least `bytes`. Contents are undefined — the span held someone
    /// else's data (or poison), so a caller that does not overwrite every span
    /// it later reads must zero it itself.
    pub fn acquire(&mut self, bytes: u32) -> Result<Block, MarshalError> {
        match self.try_acquire(bytes) {
            Some(block) => Ok(block),
            None => Err(MarshalError::new(Errno::ENOMEM, Some(&format!("pool: {bytes}B refused, the backing range is used up")), None)),
        }
    }

    /// `acquire` that returns null where a `no_spill` pool would refuse: the
    /// backing range used up, or a request larger than a region. A pool that
    /// may spill never returns null. The signal a caller acts on by reclaiming
    /// or by asking again next tick.
    pub fn try_acquire(&mut self, bytes: u32) -> Option<Block> {
        if bytes == 0 {
            return Some(DETACHED_BLOCK());
        }
        if bytes > self._stat.max_block_bytes {
            self._stat.max_block_bytes = bytes;
        }

        if self._no_spill && bytes > self.region_bytes {
            self._stat.refusals += 1;
            return None;
        }
        if self._bypass || bytes > self.region_bytes {
            // Bypassed, or larger than a region can ever hold. Its own buffer,
            // never reused — a region sized for that would waste more than the
            // spill does.
            self._stat.spills += 1;
            self._stat.live_blocks += 1;
            self._stat.live_bytes += bytes;
            let buf = ArrayBuffer::new(bytes as usize);
            self._spills.insert(buf.id());
            return Some(Block {
                buf,
                byte_offset: 0,
                bytes,
                order: -1,
                region: -1,
                free: false,
            });
        }

        let order = shift_of(bytes).max(self._min_shift) - self._min_shift;
        let block_bytes = 1u32 << (self._min_shift + order);

        // The kernel's range first, and only the kernel cuts it; past it, the
        // plain regions below, as a pool with no kernel goes past its backing.
        if let Some(kernel) = &mut self._kernel {
            let at = kernel.acquire(bytes);
            if at != 0 {
                return Some(Block {
                    buf: self._backing.clone().unwrap(),
                    byte_offset: at,
                    bytes,
                    order: order as i32,
                    region: KERNEL_REGION,
                    free: false,
                });
            }
            if self._no_spill {
                return None;
            }
        }

        for i in 0..self._regions.len() {
            let Some(region) = &mut self._regions[i] else { continue };
            let offset = region.alloc(order, &mut self._counts);
            if offset < 0 {
                continue;
            }
            region.used_bytes += block_bytes;
            self._stat.hits += 1;
            self._stat.live_blocks += 1;
            self._stat.live_bytes += bytes;
            self._stat.idle_bytes -= block_bytes;
            return Some(Block {
                buf: region.buf.clone(),
                byte_offset: region.base + offset as u32,
                bytes,
                order: order as i32,
                region: i as i32,
                free: false,
            });
        }

        // The backing first, while it has a whole region left in it; a plain
        // buffer only after. Backed regions are created first and so sit at the
        // lowest indices, which the scan above visits first — so plain regions
        // are the ones that empty out, and they are the only ones reclaimable.
        let backing = if self._kernel.is_none() { self._backing.clone() } else { None };
        let backed = backing.is_some() && self._backing_used + self.region_bytes <= self._backing_end;
        if !backed && self._no_spill {
            self._stat.refusals += 1;
            return None;
        }
        let region = if backed {
            Region::new(self._min_shift, self._max_order, backing, self._backing_used)
        } else {
            Region::new(self._min_shift, self._max_order, None, 0)
        };
        if backed {
            self._backing_used += self.region_bytes;
            self._stat.backing_used = self._backing_used - self._backing_offset;
        }
        let i = match self._regions.iter().position(Option::is_none) {
            Some(index) => index,
            None => {
                self._regions.push(None);
                self._regions.len() - 1
            }
        };
        if backed {
            self._backed.insert(i);
        } else {
            self._plain_at.insert(region.buf.id(), i);
        }
        self._regions[i] = Some(region);
        self._stat.regions += 1;
        self._stat.reserved_bytes += self.region_bytes;
        self._stat.idle_bytes += self.region_bytes;

        let region = self._regions[i].as_mut().unwrap();
        let offset = region.alloc(order, &mut self._counts);
        region.used_bytes += block_bytes;
        self._stat.misses += 1;
        self._stat.live_blocks += 1;
        self._stat.live_bytes += bytes;
        self._stat.idle_bytes -= block_bytes;
        Some(Block {
            buf: region.buf.clone(),
            byte_offset: region.base + offset as u32,
            bytes,
            order: order as i32,
            region: i as i32,
            free: false,
        })
    }

    /// Returns a block. Every view into it must have been dropped first — the
    /// next borrower writes over the same bytes, so a retained view is silent
    /// corruption rather than a stale read.
    pub fn release(&mut self, block: &mut Block) {
        if block.is_detached() {
            return;
        }
        if block.free {
            console::warn(&format!("[pool] double release of a {}B block ignored", block.bytes));
            return;
        }
        block.free = true;
        if block.region == KERNEL_REGION {
            if self._poison {
                poison(&block.buf, block.byte_offset, (1 << (self._min_shift + block.order as u32)) / 8);
            }
            self._kernel.as_mut().unwrap().release(block.byte_offset, block.bytes);
            return;
        }
        self._stat.live_blocks -= 1;
        self._stat.live_bytes -= block.bytes;
        if block.order < 0 {
            return; // a spill owns its buffer; nothing to give back
        }

        let Some(region) = self._regions[block.region as usize].as_mut() else {
            console::warn(&format!("[pool] release into a region that is already gone ({}B) ignored", block.bytes));
            return;
        };
        if self._poison {
            poison(&block.buf, block.byte_offset, (1 << (self._min_shift + block.order as u32)) / 8);
        }

        let block_bytes = 1u32 << (self._min_shift + block.order as u32);
        region.release(block.order as u32, block.byte_offset - region.base, &mut self._counts);
        region.used_bytes -= block_bytes;
        self._stat.idle_bytes += block_bytes;
        if region.used_bytes == 0 {
            self._reclaim_empty();
        }
    }

    /// Drops empty regions past the idle ceiling.
    fn _reclaim_empty(&mut self) {
        let mut empty = 0;
        for i in 0..self._regions.len() {
            let Some(region) = &self._regions[i] else { continue };
            if region.used_bytes > 0 || self._backed.contains(&i) {
                continue;
            }
            empty += 1;
            if empty <= self.max_idle_regions {
                continue;
            }
            self._plain_at.remove(&region.buf.id());
            self._regions[i] = None;
            self._stat.regions -= 1;
            self._stat.reserved_bytes -= self.region_bytes;
            self._stat.idle_bytes -= self.region_bytes;
            self._stat.evictions += 1;
        }
    }

    /// Drops every empty region. Regions with live blocks are untouched.
    pub fn clear(&mut self) {
        for i in 0..self._regions.len() {
            let Some(region) = &self._regions[i] else { continue };
            if region.used_bytes > 0 || self._backed.contains(&i) {
                continue;
            }
            self._plain_at.remove(&region.buf.id());
            self._regions[i] = None;
            self._stat.regions -= 1;
            self._stat.reserved_bytes -= self.region_bytes;
            self._stat.idle_bytes -= self.region_bytes;
        }
    }

    // ---- By address ----
    //
    // A block can also be named without its Block object: by the buffer and
    // byte offset its bytes sit at, with the size it was requested at — which
    // is all a typed view over it carries — or by a handle, one u32 packing
    // the region, the order and the page within the region. A handle is what a
    // lane holds in place of a view: numbers only, and for a region carved
    // from the backing, one step from the byte address a WASM side would use.

    /// The region an address falls in, or -1 when it is in none.
    ///
    /// A backed region's index is arithmetic, not a lookup: bases step by
    /// `region_bytes` from `_backing_offset`, and a backed region holds its
    /// slot for the pool's life (`_reclaim_empty` skips `_backed`) while a
    /// plain one is only ever cut after the backing is used up — so the nth
    /// region cut is index n. Reading the index off the address instead of a
    /// map is also what lets the range start anywhere a block can: a map keyed
    /// one way and read another silently loses every block in a range whose
    /// offset is not a whole number of regions.
    fn _locate(&self, buf: &ArrayBuffer, byte_offset: u32) -> i64 {
        if self._backing.as_ref().is_some_and(|b| buf.is(b)) {
            if byte_offset < self._backing_offset || byte_offset >= self._backing_used {
                return -1;
            }
            return ((byte_offset - self._backing_offset) / self.region_bytes) as i64;
        }
        self._plain_at.get(&buf.id()).map_or(-1, |&i| i as i64)
    }

    fn _order_of(&self, bytes: u32) -> u32 {
        shift_of(bytes).max(self._min_shift) - self._min_shift
    }

    /// The Block for `bytes` the kernel handed out at `byte_offset` to
    /// someone else — the core, acquiring on its own side — so it can be
    /// released here like any other. Zero bytes is the empty block.
    pub fn adopt(&self, byte_offset: u32, bytes: u32) -> Result<Block, MarshalError> {
        if bytes == 0 {
            return Ok(DETACHED_BLOCK());
        }
        if self._kernel.is_none() {
            return Err(MarshalError::new(Errno::EINVAL, Some("pool: only a pool with a kernel adopts a block"), None));
        }
        Ok(Block {
            buf: self._backing.clone().unwrap(),
            byte_offset,
            bytes,
            order: self._order_of(bytes) as i32,
            region: KERNEL_REGION,
            free: false,
        })
    }

    /// Returns the block of `bytes` at this address — what `release` does for
    /// a Block, for a caller holding only a view.
    pub fn release_at(&mut self, buf: &ArrayBuffer, byte_offset: u32, bytes: u32) {
        if bytes == 0 {
            return;
        }
        if let (Some(kernel), true) = (self._kernel.as_mut(), self._backing.as_ref().is_some_and(|b| buf.is(b))) {
            if self._poison {
                poison(buf, byte_offset, (1 << (self._min_shift + shift_of(bytes).max(self._min_shift) - self._min_shift)) / 8);
            }
            if !kernel.release(byte_offset, bytes) {
                console::warn(&format!("[pool] release of a {bytes}B block the kernel does not hold, or holds already, ignored"));
            }
            return;
        }
        let i = self._locate(buf, byte_offset);
        if i < 0 {
            if !self._spills.contains(&buf.id()) {
                console::warn(&format!("[pool] release of {bytes}B from a buffer this pool never lent ignored"));
                return;
            }
            self._spills.remove(&buf.id());
            self._stat.live_blocks -= 1;
            self._stat.live_bytes -= bytes;
            return;
        }
        let order = self._order_of(bytes);
        let min_shift = self._min_shift;
        let poisoned = self._poison;
        let region = self._regions[i as usize].as_mut().unwrap();
        let offset = byte_offset - region.base;
        if region.is_block_free(order, offset) {
            console::warn(&format!("[pool] double release of a {bytes}B block ignored"));
            return;
        }
        self._stat.live_blocks -= 1;
        self._stat.live_bytes -= bytes;
        let block_bytes = 1u32 << (min_shift + order);
        if poisoned {
            poison(buf, byte_offset, block_bytes / 8);
        }
        region.release(order, offset, &mut self._counts);
        region.used_bytes -= block_bytes;
        self._stat.idle_bytes += block_bytes;
        if region.used_bytes == 0 {
            self._reclaim_empty();
        }
    }

    /// The handle for the block of `bytes` at this address. A spill has none:
    /// it is not in a region.
    pub fn handle_at(&self, buf: &ArrayBuffer, byte_offset: u32, bytes: u32) -> Result<u32, MarshalError> {
        if self._kernel.is_some() && self._backing.as_ref().is_some_and(|b| buf.is(b)) {
            return Ok(KERNEL_HANDLE | byte_offset);
        }
        let i = self._locate(buf, byte_offset);
        if i < 0 {
            return Err(MarshalError::new(Errno::EINVAL, Some(&format!("pool: a {bytes}B block outside every region has no handle")), None));
        }
        if i >= 1i64 << (32 - self._order_bits - self._max_order) {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("pool: region {i} does not fit a handle")), None));
        }
        let page = (byte_offset - self._regions[i as usize].as_ref().unwrap().base) >> self._min_shift;
        // In u64: the region's shift is 32 when it can only be region 0.
        Ok((((i as u64) << (self._order_bits + self._max_order)) as u32) | (self._order_of(bytes) << self._max_order) | page)
    }

    /// Where the block a handle names starts.
    pub fn handle_base(&self, handle: u32) -> (ArrayBuffer, u32) {
        if (handle & KERNEL_HANDLE) != 0 {
            return (self._backing.clone().unwrap(), handle & !KERNEL_HANDLE);
        }
        let i = (handle >> (self._order_bits + self._max_order)) as usize;
        let region = self._regions[i].as_ref().unwrap();
        let page = handle & ((1 << self._max_order) - 1);
        (region.buf.clone(), region.base + (page << self._min_shift))
    }
}

/// `new Float64Array(buf, byte_offset, doubles).fill(NaN)`.
fn poison(buf: &ArrayBuffer, byte_offset: u32, doubles: u32) {
    TypedArray::<f64>::over(buf.clone(), byte_offset as usize, doubles as usize).fill(f64::NAN);
}

/// The backed side of a pool, run here: a pool over a backing range with
/// `no_spill`, seen by address — what the TypeScript pool's `kernel` names
/// when the range is the module's.
impl PoolKernel for BuddyAllocator {
    fn acquire(&mut self, bytes: u32) -> u32 {
        match self.try_acquire(bytes) {
            Some(block) if !block.is_detached() => block.byte_offset,
            _ => 0,
        }
    }

    fn release(&mut self, at: u32, bytes: u32) -> bool {
        let Some(backing) = self._backing.clone() else { return false };
        if self._locate(&backing, at) < 0 || PoolKernel::is_free(self, at, bytes) {
            return false;
        }
        self.release_at(&backing, at, bytes);
        true
    }

    /// Free at its own order, or inside a free block of a larger order — a
    /// released block merges upward, and "holds already" has to see it there.
    fn is_free(&self, at: u32, bytes: u32) -> bool {
        let Some(backing) = &self._backing else { return false };
        let i = self._locate(backing, at);
        if i < 0 {
            return false;
        }
        let region = self._regions[i as usize].as_ref().unwrap();
        let offset = at - region.base;
        (self._order_of(bytes)..=self._max_order).any(|k| region.is_block_free(k, offset))
    }

    fn stat(&self) -> [u32; 12] {
        let s = self.stat();
        [
            s.live_bytes,
            s.idle_bytes,
            s.reserved_bytes,
            s.regions,
            s.live_blocks,
            s.hits,
            s.misses,
            s.splits,
            s.merges,
            s.max_block_bytes,
            s.backing_used,
            s.refusals,
        ]
    }
}

#[cfg(test)]
#[path = "buddy.test.rs"]
mod test;
