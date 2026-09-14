// @xpute/core/ds/buddy.ts

/**
 * Buddy allocator for buffers whose deaths are unordered.
 *
 * The sibling of arena.ts, for the lifetime arena.ts structurally cannot
 * serve. Arena/Vector are bump allocators: everything they hand out dies at
 * one rewind, so they fit "build it, consume it, drop it all" exactly. A
 * retained buffer — one an owner holds for as long as *it* lives, released
 * whenever that owner happens to die — has no such common rewind point, and
 * a bump cursor can never reclaim the middle of its own range.
 *
 * Memory comes from a list of fixed-size **regions**, each one ArrayBuffer
 * carved by the classic buddy scheme: a request rounds up to a power-of-two
 * order, a free block of a larger order splits in half repeatedly to serve
 * it, and a released block merges back with its buddy whenever that buddy is
 * also free. Allocation and release are both O(orders).
 *
 * **Why buddy and not plain per-class free lists.** Plain free lists were
 * tried first and measured: a 17-36% hit rate with the lists sitting full of
 * blocks exactly one order above every request. The reason is structural
 * rather than incidental — the workload migrates between classes by
 * definition. A renderer dropping a chunk's fine LOD tier and asking for a
 * coarse one in the same breath frees a large block and requests a small
 * one; a zoom back in does the reverse. Per-class lists cannot move bytes
 * between classes at all, so every such transition is a miss on one side and
 * dead inventory on the other. Splitting fixes one direction, merging fixes
 * the other, and needing both is what makes this a buddy allocator rather
 * than a pool with a hack in it.
 *
 * Byte blocks, not typed arrays, so one logically-single buffer with several
 * differently-typed spans (a mesh's position/normal/index/...) is one
 * allocation rather than one per span. Callers carve their own views from
 * `buf` at `byte_offset`; the minimum order is a multiple of 8 bytes and
 * every order is a power-of-two multiple of it, so any view alignment up to
 * f64 is satisfied without the caller thinking about it.
 *
 * Deliberately portable. In Rust each region is a `Box<[u8]>` (or one big
 * `Vec<u8>` under WASM) and everything above is index arithmetic that
 * translates line for line. Under WASM, where linear memory grows and never
 * shrinks, buddy-over-one-region is the canonical shape rather than a
 * nicety, and `max_idle_regions` is the knob that decides whether a peak
 * becomes permanent residency.
 *
 * The one failure mode this introduces is a view outliving its release:
 * another borrower gets those exact bytes, so it is silent corruption rather
 * than a stale read. `poison` makes it loud, and the double-release guard is
 * always on.
 */

import type { u32 } from "../kit/word.ts";
import { MarshalError } from "@xpute/core/sys/error.ts";
import { Errno } from "@xpute/core/sys/errno.ts";

export type BuddyOptions = {
  /** Smallest allocation, and the alignment every block satisfies. */
  min_block_bytes?: u32;
  /** One region's size. A request larger than this spills to its own ArrayBuffer. */
  region_bytes?: u32;
  /**
   * How many fully-empty regions to keep rather than release to the
   * collector. Borrowed bytes are not a surplus to reclaim — refusing to
   * retain them frees nothing and only turns every release into a free — so
   * what a ceiling can honestly bound is the idle surplus, and with buddy
   * that surplus is countable in whole empty regions.
   */
  max_idle_regions?: u32;
  /** Fill released blocks with NaN so a stale reader is loud. Dev only — it is a memset per release. */
  poison?: boolean;
  /**
   * Hand every request its own buffer and reuse nothing, so a caller whose
   * ownership is not yet airtight cannot corrupt one borrower with another's
   * writes. The allocator becomes a plain `new ArrayBuffer` with bookkeeping
   * — every acquire counts as a spill — which is the *previous* behaviour of
   * whatever it replaced, and therefore always a safe place to stand.
   *
   * A switch worth having: pooling converts a use-after-free from a harmless
   * stale read into silent cross-borrower corruption, so "is the pool at
   * fault" has to be answerable by flipping one flag rather than by reverting
   * a change.
   */
  bypass?: boolean;
  /**
   * A buffer to carve regions out of before allocating any of their own —
   * typically a fixed-length `WebAssembly.Memory`'s buffer, so JS and a
   * future WASM side address the same bytes.
   *
   * Regions are cut from it front to back, one `region_bytes` at a time, and
   * are never handed back: the bytes belong to the backing, not to the
   * collector, so releasing them would free nothing. They sit idle and are
   * reused instead, and `max_idle_regions` governs only the plain regions
   * allocated once the backing is used up. The backing must never be
   * resized — a view into a detached buffer reads as length zero.
   */
  backing?: ArrayBuffer;
  /**
   * The range of the backing this pool may carve: a start offset and a byte
   * length, both multiples of `region_bytes`. Several pools over one memory
   * take disjoint ranges — the sections of a core memory — and none can
   * reach another's. Defaults to the whole backing.
   */
  backing_offset?: u32;
  backing_bytes?: u32;
  /**
   * Refuse rather than spill: once the backing range is used up, `acquire`
   * returns null instead of opening a plain buffer, and a request larger
   * than a region returns null too. For a pool whose bytes a kernel over the
   * backing must be able to see — nothing it lends may live outside it.
   */
  no_spill?: boolean;
  /**
   * A kernel that owns the backing range: the buddy over it runs there,
   * and this pool only names its blocks — an address in, an address out —
   * and keeps what the kernel cannot do, the plain regions past the range.
   * Given with `backing`, and the range is the kernel's to cut.
   */
  kernel?: PoolKernel;
};

/**
 * The backed side of a pool, run elsewhere (crates/xpute-core/src/ds/buddy.rs
 * through pxl-wasm's pool exports): blocks named by their address in the
 * backing. `acquire` answers 0 for a refusal; `release` false for a block
 * the kernel does not hold or holds already. `stat` is the kernel's count
 * of what it did, in BuddyStat's first twelve fields' order.
 */
export type PoolKernel = {
  acquire(bytes: u32): u32;
  release(at: u32, bytes: u32): boolean;
  is_free(at: u32, bytes: u32): boolean;
  stat(): ArrayLike<u32>;
};

/** The `region` of a block the kernel lent: it is in the backing, and the
 * kernel is the one to ask about it. */
const KERNEL_REGION = -2;

/** The handle of a kernel block: its address, and the high bit to say so
 * (no backing is 2 GB). A block in a plain region keeps the packed form. */
const KERNEL_HANDLE: u32 = 0x8000_0000;

/**
 * One borrowed span: `bytes` of `buf` starting at `byte_offset`. The region
 * behind `buf` is shared with other live blocks, so a view must always be
 * built from `byte_offset` with an explicit length, never from `buf` alone.
 */
export type Block = {
  readonly buf: ArrayBuffer;
  readonly byte_offset: u32;
  readonly bytes: u32;
  /** Buddy order, or -1 for a spill/empty block that owns its whole buffer. */
  readonly order: number;
  /** Which region `buf` belongs to; -1 alongside order -1. */
  readonly region: number;
  /** Guard state, not a capability: a released block's views stay reachable. */
  free: boolean;
};

export type BuddyStat = {
  /** Bytes in blocks currently borrowed (as requested, before rounding up). */
  live_bytes: u32;
  /** Region bytes not currently carved into a live block. */
  idle_bytes: u32;
  /** Total bytes across all regions. */
  reserved_bytes: u32;
  regions: u32;
  live_blocks: u32;
  /** Acquires served from the existing regions. */
  hits: u32;
  /** Acquires that had to open a new region. */
  misses: u32;
  /** Acquires larger than one region, given their own buffer and never reused. */
  spills: u32;
  /** Empty regions handed back to the collector. */
  evictions: u32;
  /** Splits and merges performed — the work buddy does that free lists cannot. */
  splits: u32;
  /** How many merges happened; a healthy steady state keeps this near `splits`. */
  merges: u32;
  /** Largest single request seen. What region_bytes has to be chosen against:
   * below it, every such request spills and is never reused. */
  max_block_bytes: u32;
  /** Bytes of the backing range cut into regions so far, and the range's
   * size — how close the pool is to its first spill or refusal. */
  backing_used: u32;
  backing_bytes: u32;
  /** Acquires refused under `no_spill`. */
  refusals: u32;
};

/**
 * The zero-size block, and equally the stand-in for a buffer set built
 * outside any pool (a test fixture, a slice taken for inspection): it owns
 * nothing, so releasing it is a no-op rather than an error.
 */
export const DETACHED_BLOCK: Block = { buf: new ArrayBuffer(0), byte_offset: 0, bytes: 0, order: -1, region: -1, free: false };

function shift_of(bytes: u32): u32 {
  return 32 - Math.clz32(bytes - 1);
}

/**
 * One buddy-carved ArrayBuffer.
 *
 * The free blocks of each order are a doubly linked list threaded through
 * the free blocks themselves — the first two words of a free block hold the
 * offsets of the next and previous free block of its order — which is the
 * classic shape and the one that costs nothing: no container per order, no
 * iterator to take the head, no allocation anywhere in the allocator. A
 * block is only written to while it is free, and a borrower gets its bytes
 * dirty in any case. `is_free` answers "is the block of order k at this
 * offset free" in O(1) — the buddy test on release needs exactly that
 * question, and it cannot be read off the list without walking it.
 *
 * Offsets on the lists are region-relative and word-aligned by
 * construction (the minimum order is at least eight bytes); NIL marks a
 * list's end and a block's missing neighbour.
 */
const NIL = 0xffffffff;

class Region {
  readonly buf: ArrayBuffer;
  /** The region's bytes as words, for the list links inside free blocks. */
  private readonly words: Uint32Array;
  /** Offset of the first free block of each order, or NIL. */
  private readonly head: Uint32Array;
  private readonly is_free: Uint8Array[];
  /** Bytes carved into live blocks; a region with 0 is fully reclaimable. */
  used_bytes: u32 = 0;

  /**
   * `base` is where this region starts inside `buf`: zero for a region that
   * owns its buffer, and some multiple of the region size for one carved from
   * a shared backing. Offsets inside the region — the ones the buddy
   * arithmetic works in — are relative to it; a block's `byte_offset` is
   * absolute, so it can index `buf` directly.
   */
  constructor(readonly min_shift: u32, readonly max_order: u32, backing: ArrayBuffer | null = null, readonly base: u32 = 0) {
    const bytes = 1 << (min_shift + max_order);
    this.buf = backing ?? new ArrayBuffer(bytes);
    this.words = new Uint32Array(this.buf, base, bytes >>> 2);
    this.head = new Uint32Array(max_order + 1).fill(NIL);
    this.is_free = [];
    for (let k = 0; k <= max_order; k++) this.is_free.push(new Uint8Array(1 << (max_order - k)));
    this.mark_free(max_order, 0);
  }

  private mark_free(order: u32, offset: u32): void {
    const words = this.words;
    const w = offset >>> 2;
    const next = this.head[order];
    words[w] = next;
    words[w + 1] = NIL;
    if (next !== NIL) words[(next >>> 2) + 1] = offset;
    this.head[order] = offset;
    this.is_free[order][offset >>> (this.min_shift + order)] = 1;
  }

  private mark_used(order: u32, offset: u32): void {
    const words = this.words;
    const w = offset >>> 2;
    const next = words[w];
    const prev = words[w + 1];
    if (prev === NIL) this.head[order] = next;
    else words[prev >>> 2] = next;
    if (next !== NIL) words[(next >>> 2) + 1] = prev;
    this.is_free[order][offset >>> (this.min_shift + order)] = 0;
  }

  /** Offset of a free block of exactly `order`, splitting a larger one if
   * needed; -1 when the region cannot serve it. Returns splits performed via
   * `counts`, which the pool folds into its own stats. */
  alloc(order: u32, counts: { splits: u32 }): number {
    let k = order;
    while (k <= this.max_order && this.head[k] === NIL) k++;
    if (k > this.max_order) return -1;

    const offset = this.head[k];
    this.mark_used(k, offset);
    while (k > order) {
      k--;
      counts.splits++;
      this.mark_free(k, offset + (1 << (this.min_shift + k)));
    }
    return offset;
  }

  /** Whether the block of `order` at `offset` is itself on a free list. */
  is_block_free(order: u32, offset: u32): boolean {
    return this.is_free[order][offset >>> (this.min_shift + order)] === 1;
  }

  /** Returns a block and merges it upward as far as its buddies allow. */
  release(order: u32, offset: u32, counts: { merges: u32 }): void {
    let k = order;
    let at = offset;
    while (k < this.max_order) {
      const buddy = at ^ (1 << (this.min_shift + k));
      if (this.is_free[k][buddy >>> (this.min_shift + k)] !== 1) break;
      this.mark_used(k, buddy);
      counts.merges++;
      at = Math.min(at, buddy);
      k++;
    }
    this.mark_free(k, at);
  }
}

export class BuddyAllocator {
  readonly min_block_bytes: u32;
  readonly region_bytes: u32;
  readonly max_idle_regions: u32;

  /** The pool's own counts: with a kernel, the plain side only — `stat`
   * folds the kernel's in. */
  private readonly _stat: BuddyStat = {
    live_bytes: 0,
    idle_bytes: 0,
    reserved_bytes: 0,
    regions: 0,
    live_blocks: 0,
    hits: 0,
    misses: 0,
    spills: 0,
    evictions: 0,
    splits: 0,
    merges: 0,
    max_block_bytes: 0,
    backing_used: 0,
    backing_bytes: 0,
    refusals: 0,
  };

  /** What the pool has done, both sides together when a kernel holds the
   * backed one. Read fresh each time. */
  get stat(): Readonly<BuddyStat> {
    const own = this._stat;
    const k = this._kernel;
    if (k === null) return own;
    const ks = k.stat();
    return {
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
      max_block_bytes: Math.max(own.max_block_bytes, ks[9]),
      backing_used: ks[10],
      backing_bytes: own.backing_bytes,
      // The kernel refuses past its range whatever the pool's policy; for a
      // pool that may spill, that is where a plain region begins, not a
      // refusal the caller saw.
      refusals: own.refusals + (this._no_spill ? ks[11] : 0),
    };
  }

  private readonly _min_shift: u32;
  private readonly _max_order: u32;
  private readonly _regions: (Region | null)[] = [];
  private readonly _poison: boolean;
  private readonly _bypass: boolean;
  private readonly _no_spill: boolean;
  private readonly _backing: ArrayBuffer | null;
  private readonly _kernel: PoolKernel | null;
  /** The backing range: where it starts, and how far into it regions have
   * been cut. `_backing_used` is an absolute offset. */
  private readonly _backing_offset: u32;
  private readonly _backing_end: u32;
  private _backing_used: u32;
  /** Indices of regions carved from the backing — never reclaimed. */
  private readonly _backed = new Set<number>();
  /** Region index by where it starts in the backing, and by its own buffer —
   * how an address (a buffer and a byte offset) finds its region. */
  private readonly _backed_at = new Map<number, number>();
  private readonly _plain_at = new Map<ArrayBuffer, number>();
  /** Buffers handed out whole — a spill is released by address too. */
  private readonly _spills = new WeakSet<ArrayBuffer>();
  /** Bits a handle spends on the order. */
  private readonly _order_bits: u32;

  constructor(opts: BuddyOptions = {}) {
    const {
      min_block_bytes = 1 << 12,
      region_bytes = 1 << 26,
      max_idle_regions = 2,
      poison = false,
      bypass = false,
      backing = null,
      no_spill = false,
      kernel = null,
    } = opts;
    if (min_block_bytes < 8 || region_bytes < min_block_bytes) {
      throw new MarshalError(Errno.EINVAL, `pool: bad region shape min=${min_block_bytes} region=${region_bytes}`);
    }
    this._min_shift = shift_of(min_block_bytes);
    this.min_block_bytes = 1 << this._min_shift;
    this._max_order = shift_of(region_bytes) - this._min_shift;
    this.region_bytes = 1 << (this._min_shift + this._max_order);
    this.max_idle_regions = max_idle_regions;
    this._poison = poison;
    this._bypass = bypass;
    this._no_spill = no_spill;
    this._backing = backing;
    if (kernel !== null && backing === null) throw new MarshalError(Errno.EINVAL, "pool: a kernel needs the backing it runs over");
    this._kernel = kernel;
    // An explicit range must be whole regions; the default is however many
    // whole regions the backing holds past the offset, which for a backing
    // smaller than a region is none.
    const offset = opts.backing_offset ?? 0;
    const whole = backing === null ? 0 : Math.max(0, backing.byteLength - offset) - (Math.max(0, backing.byteLength - offset) % this.region_bytes);
    const bytes = opts.backing_bytes ?? whole;
    if (backing !== null) {
      if (offset % this.region_bytes !== 0 || bytes % this.region_bytes !== 0 || offset + bytes > backing.byteLength) {
        throw new MarshalError(Errno.EINVAL, `pool: backing range ${offset}+${bytes} is not whole regions inside ${backing.byteLength}`);
      }
    }
    this._backing_offset = offset;
    this._backing_used = offset;
    this._backing_end = backing !== null ? offset + bytes : 0;
    this._stat.backing_bytes = backing !== null ? bytes : 0;
    if (no_spill && backing === null) throw new MarshalError(Errno.EINVAL, "pool: no_spill needs a backing to refuse past");
    this._order_bits = shift_of(this._max_order + 2);
  }

  /**
   * Borrows at least `bytes`. Contents are undefined — the span held someone
   * else's data (or poison), so a caller that does not overwrite every span
   * it later reads must zero it itself.
   */
  acquire(bytes: u32): Block {
    const block = this.try_acquire(bytes);
    if (block === null) throw new MarshalError(Errno.ENOMEM, `pool: ${bytes}B refused, the backing range is used up`);
    return block;
  }

  /**
   * `acquire` that returns null where a `no_spill` pool would refuse: the
   * backing range used up, or a request larger than a region. A pool that
   * may spill never returns null. The signal a caller acts on by reclaiming
   * or by asking again next tick.
   */
  try_acquire(bytes: u32): Block | null {
    if (bytes <= 0) return DETACHED_BLOCK;
    if (bytes > this._stat.max_block_bytes) this._stat.max_block_bytes = bytes;

    if (this._no_spill && bytes > this.region_bytes) {
      this._stat.refusals++;
      return null;
    }
    if (this._bypass || bytes > this.region_bytes) {
      // Bypassed, or larger than a region can ever hold. Its own buffer,
      // never reused — a region sized for that would waste more than the
      // spill does.
      this._stat.spills++;
      this._stat.live_blocks++;
      this._stat.live_bytes += bytes;
      const buf = new ArrayBuffer(bytes);
      this._spills.add(buf);
      return { buf, byte_offset: 0, bytes, order: -1, region: -1, free: false };
    }

    const order = Math.max(shift_of(bytes), this._min_shift) - this._min_shift;
    const block_bytes = 1 << (this._min_shift + order);

    // The kernel's range first, and only the kernel cuts it; past it, the
    // plain regions below, as a pool with no kernel goes past its backing.
    if (this._kernel !== null) {
      const at = this._kernel.acquire(bytes);
      if (at !== 0) return { buf: this._backing!, byte_offset: at, bytes, order, region: KERNEL_REGION, free: false };
      if (this._no_spill) return null;
    }

    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region) continue;
      const offset = region.alloc(order, this._stat);
      if (offset < 0) continue;
      region.used_bytes += block_bytes;
      this._stat.hits++;
      this._stat.live_blocks++;
      this._stat.live_bytes += bytes;
      this._stat.idle_bytes -= block_bytes;
      return { buf: region.buf, byte_offset: region.base + offset, bytes, order, region: i, free: false };
    }

    // The backing first, while it has a whole region left in it; a plain
    // buffer only after. Backed regions are created first and so sit at the
    // lowest indices, which the scan above visits first — so plain regions
    // are the ones that empty out, and they are the only ones reclaimable.
    const backing = this._kernel === null ? this._backing : null;
    const backed = backing !== null && this._backing_used + this.region_bytes <= this._backing_end;
    if (!backed && this._no_spill) {
      this._stat.refusals++;
      return null;
    }
    const region = backed ? new Region(this._min_shift, this._max_order, backing, this._backing_used) : new Region(this._min_shift, this._max_order);
    if (backed) {
      this._backing_used += this.region_bytes;
      this._stat.backing_used = this._backing_used - this._backing_offset;
    }
    const index = this._regions.indexOf(null);
    const i = index >= 0 ? index : this._regions.length;
    this._regions[i] = region;
    if (backed) {
      this._backed.add(i);
      this._backed_at.set(region.base, i);
    } else {
      this._plain_at.set(region.buf, i);
    }
    this._stat.regions++;
    this._stat.reserved_bytes += this.region_bytes;
    this._stat.idle_bytes += this.region_bytes;

    const offset = region.alloc(order, this._stat);
    region.used_bytes += block_bytes;
    this._stat.misses++;
    this._stat.live_blocks++;
    this._stat.live_bytes += bytes;
    this._stat.idle_bytes -= block_bytes;
    return { buf: region.buf, byte_offset: region.base + offset, bytes, order, region: i, free: false };
  }

  /**
   * Returns a block. Every view into it must have been dropped first — the
   * next borrower writes over the same bytes, so a retained view is silent
   * corruption rather than a stale read.
   */
  release(block: Block): void {
    if (block === DETACHED_BLOCK) return;
    if (block.free) {
      console.warn(`[pool] double release of a ${block.bytes}B block ignored`);
      return;
    }
    block.free = true;
    if (block.region === KERNEL_REGION) {
      if (this._poison) new Float64Array(block.buf, block.byte_offset, (1 << (this._min_shift + block.order)) / 8).fill(NaN);
      this._kernel!.release(block.byte_offset, block.bytes);
      return;
    }
    this._stat.live_blocks--;
    this._stat.live_bytes -= block.bytes;
    if (block.order < 0) return; // a spill owns its buffer; nothing to give back

    const region = this._regions[block.region];
    if (!region) {
      console.warn(`[pool] release into a region that is already gone (${block.bytes}B) ignored`);
      return;
    }
    if (this._poison) new Float64Array(block.buf, block.byte_offset, (1 << (this._min_shift + block.order)) / 8).fill(NaN);

    const block_bytes = 1 << (this._min_shift + block.order);
    region.release(block.order, block.byte_offset - region.base, this._stat);
    region.used_bytes -= block_bytes;
    this._stat.idle_bytes += block_bytes;
    if (region.used_bytes === 0) this._reclaim_empty();
  }

  /** Drops empty regions past the idle ceiling. */
  private _reclaim_empty(): void {
    let empty = 0;
    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region || region.used_bytes > 0 || this._backed.has(i)) continue;
      if (++empty <= this.max_idle_regions) continue;
      this._plain_at.delete(region.buf);
      this._regions[i] = null;
      this._stat.regions--;
      this._stat.reserved_bytes -= this.region_bytes;
      this._stat.idle_bytes -= this.region_bytes;
      this._stat.evictions++;
    }
  }

  /** Drops every empty region. Regions with live blocks are untouched. */
  clear(): void {
    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region || region.used_bytes > 0 || this._backed.has(i)) continue;
      this._plain_at.delete(region.buf);
      this._regions[i] = null;
      this._stat.regions--;
      this._stat.reserved_bytes -= this.region_bytes;
      this._stat.idle_bytes -= this.region_bytes;
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

  /** The region an address falls in, or -1 when it is in none. */
  private _locate(buf: ArrayBuffer, byte_offset: u32): number {
    if (buf === this._backing) return this._backed_at.get(byte_offset - (byte_offset % this.region_bytes)) ?? -1;
    return this._plain_at.get(buf) ?? -1;
  }

  private _order_of(bytes: u32): u32 {
    return Math.max(shift_of(bytes), this._min_shift) - this._min_shift;
  }

  /** The Block for `bytes` the kernel handed out at `byte_offset` to
   * someone else — the core, acquiring on its own side — so it can be
   * released here like any other. Zero bytes is the empty block. */
  adopt(byte_offset: u32, bytes: u32): Block {
    if (bytes <= 0) return DETACHED_BLOCK;
    if (this._kernel === null) throw new MarshalError(Errno.EINVAL, "pool: only a pool with a kernel adopts a block");
    return { buf: this._backing!, byte_offset, bytes, order: this._order_of(bytes), region: KERNEL_REGION, free: false };
  }

  /** Returns the block of `bytes` at this address — what `release` does for
   * a Block, for a caller holding only a view. */
  release_at(buf: ArrayBuffer, byte_offset: u32, bytes: u32): void {
    if (bytes <= 0) return;
    if (this._kernel !== null && buf === this._backing) {
      if (this._poison) new Float64Array(buf, byte_offset, (1 << (this._min_shift + this._order_of(bytes))) / 8).fill(NaN);
      if (!this._kernel.release(byte_offset, bytes)) console.warn(`[pool] release of a ${bytes}B block the kernel does not hold, or holds already, ignored`);
      return;
    }
    const i = this._locate(buf, byte_offset);
    if (i < 0) {
      if (!this._spills.has(buf)) {
        console.warn(`[pool] release of ${bytes}B from a buffer this pool never lent ignored`);
        return;
      }
      this._spills.delete(buf);
      this._stat.live_blocks--;
      this._stat.live_bytes -= bytes;
      return;
    }
    const region = this._regions[i]!;
    const order = this._order_of(bytes);
    const offset = byte_offset - region.base;
    if (region.is_block_free(order, offset)) {
      console.warn(`[pool] double release of a ${bytes}B block ignored`);
      return;
    }
    this._stat.live_blocks--;
    this._stat.live_bytes -= bytes;
    const block_bytes = 1 << (this._min_shift + order);
    if (this._poison) new Float64Array(buf, byte_offset, block_bytes / 8).fill(NaN);
    region.release(order, offset, this._stat);
    region.used_bytes -= block_bytes;
    this._stat.idle_bytes += block_bytes;
    if (region.used_bytes === 0) this._reclaim_empty();
  }

  /** The handle for the block of `bytes` at this address. A spill has none:
   * it is not in a region. */
  handle_at(buf: ArrayBuffer, byte_offset: u32, bytes: u32): u32 {
    if (this._kernel !== null && buf === this._backing) return (KERNEL_HANDLE | byte_offset) >>> 0;
    const i = this._locate(buf, byte_offset);
    if (i < 0) throw new MarshalError(Errno.EINVAL, `pool: a ${bytes}B block outside every region has no handle`);
    // 2 ** rather than 1 <<: the exponent reaches 31 (a region of one minimum
    // block), where 1 << 31 is negative and every region would be refused.
    if (i >= 2 ** (32 - this._order_bits - this._max_order)) throw new MarshalError(Errno.EOVERFLOW, `pool: region ${i} does not fit a handle`);
    const page = (byte_offset - this._regions[i]!.base) >>> this._min_shift;
    return ((i << (this._order_bits + this._max_order)) | (this._order_of(bytes) << this._max_order) | page) >>> 0;
  }

  /** Where the block a handle names starts. */
  handle_base(handle: u32): { buf: ArrayBuffer; byte_offset: u32 } {
    if ((handle & KERNEL_HANDLE) !== 0) return { buf: this._backing!, byte_offset: (handle & ~KERNEL_HANDLE) >>> 0 };
    const i = handle >>> (this._order_bits + this._max_order);
    const region = this._regions[i]!;
    const page = handle & ((1 << this._max_order) - 1);
    return { buf: region.buf, byte_offset: region.base + (page << this._min_shift) };
  }
}
