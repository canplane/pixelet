// @xpute/core/ds/pool.ts

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

export type PoolOptions = {
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
};

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

export type PoolStat = {
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
 * `free[k]` holds the offsets of every free block of order k, and `is_free`
 * answers "is the block of order k at this offset free" in O(1) — the buddy
 * test on release needs exactly that question, and a linear scan of `free[k]`
 * would make release O(free blocks) instead of O(orders).
 */
class Region {
  readonly buf: ArrayBuffer;
  readonly free: Set<u32>[];
  private readonly is_free: Uint8Array[];
  /** Bytes carved into live blocks; a region with 0 is fully reclaimable. */
  used_bytes: u32 = 0;

  constructor(readonly min_shift: u32, readonly max_order: u32) {
    this.buf = new ArrayBuffer(1 << (min_shift + max_order));
    this.free = [];
    this.is_free = [];
    for (let k = 0; k <= max_order; k++) {
      this.free.push(new Set());
      this.is_free.push(new Uint8Array(1 << (max_order - k)));
    }
    this.mark_free(max_order, 0);
  }

  private mark_free(order: u32, offset: u32): void {
    this.free[order].add(offset);
    this.is_free[order][offset >>> (this.min_shift + order)] = 1;
  }

  private mark_used(order: u32, offset: u32): void {
    this.free[order].delete(offset);
    this.is_free[order][offset >>> (this.min_shift + order)] = 0;
  }

  /** Offset of a free block of exactly `order`, splitting a larger one if
   * needed; -1 when the region cannot serve it. Returns splits performed via
   * `counts`, which the pool folds into its own stats. */
  alloc(order: u32, counts: { splits: u32 }): number {
    let k = order;
    while (k <= this.max_order && this.free[k].size === 0) k++;
    if (k > this.max_order) return -1;

    const offset = this.free[k].values().next().value as u32;
    this.mark_used(k, offset);
    while (k > order) {
      k--;
      counts.splits++;
      this.mark_free(k, offset + (1 << (this.min_shift + k)));
    }
    return offset;
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

export class Pool {
  readonly min_block_bytes: u32;
  readonly region_bytes: u32;
  readonly max_idle_regions: u32;

  readonly stat: PoolStat = {
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
  };

  private readonly _min_shift: u32;
  private readonly _max_order: u32;
  private readonly _regions: (Region | null)[] = [];
  private readonly _poison: boolean;
  private readonly _bypass: boolean;

  constructor(opts: PoolOptions = {}) {
    const {
      min_block_bytes = 1 << 12,
      region_bytes = 1 << 26,
      max_idle_regions = 2,
      poison = false,
      bypass = false,
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
  }

  /**
   * Borrows at least `bytes`. Contents are undefined — the span held someone
   * else's data (or poison), so a caller that does not overwrite every span
   * it later reads must zero it itself.
   */
  acquire(bytes: u32): Block {
    if (bytes <= 0) return DETACHED_BLOCK;
    if (bytes > this.stat.max_block_bytes) this.stat.max_block_bytes = bytes;

    if (this._bypass || bytes > this.region_bytes) {
      // Bypassed, or larger than a region can ever hold. Its own buffer,
      // never reused — a region sized for that would waste more than the
      // spill does.
      this.stat.spills++;
      this.stat.live_blocks++;
      this.stat.live_bytes += bytes;
      return { buf: new ArrayBuffer(bytes), byte_offset: 0, bytes, order: -1, region: -1, free: false };
    }

    const order = Math.max(shift_of(bytes), this._min_shift) - this._min_shift;
    const block_bytes = 1 << (this._min_shift + order);

    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region) continue;
      const offset = region.alloc(order, this.stat);
      if (offset < 0) continue;
      region.used_bytes += block_bytes;
      this.stat.hits++;
      this.stat.live_blocks++;
      this.stat.live_bytes += bytes;
      this.stat.idle_bytes -= block_bytes;
      return { buf: region.buf, byte_offset: offset, bytes, order, region: i, free: false };
    }

    const region = new Region(this._min_shift, this._max_order);
    const index = this._regions.indexOf(null);
    const i = index >= 0 ? index : this._regions.length;
    this._regions[i] = region;
    this.stat.regions++;
    this.stat.reserved_bytes += this.region_bytes;
    this.stat.idle_bytes += this.region_bytes;

    const offset = region.alloc(order, this.stat);
    region.used_bytes += block_bytes;
    this.stat.misses++;
    this.stat.live_blocks++;
    this.stat.live_bytes += bytes;
    this.stat.idle_bytes -= block_bytes;
    return { buf: region.buf, byte_offset: offset, bytes, order, region: i, free: false };
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
    this.stat.live_blocks--;
    this.stat.live_bytes -= block.bytes;
    if (block.order < 0) return; // a spill owns its buffer; nothing to give back

    const region = this._regions[block.region];
    if (!region) {
      console.warn(`[pool] release into a region that is already gone (${block.bytes}B) ignored`);
      return;
    }
    if (this._poison) new Float64Array(block.buf, block.byte_offset, (1 << (this._min_shift + block.order)) / 8).fill(NaN);

    const block_bytes = 1 << (this._min_shift + block.order);
    region.release(block.order, block.byte_offset, this.stat);
    region.used_bytes -= block_bytes;
    this.stat.idle_bytes += block_bytes;
    if (region.used_bytes === 0) this._reclaim_empty();
  }

  /** Drops empty regions past the idle ceiling. */
  private _reclaim_empty(): void {
    let empty = 0;
    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region || region.used_bytes > 0) continue;
      if (++empty <= this.max_idle_regions) continue;
      this._regions[i] = null;
      this.stat.regions--;
      this.stat.reserved_bytes -= this.region_bytes;
      this.stat.idle_bytes -= this.region_bytes;
      this.stat.evictions++;
    }
  }

  /** Drops every empty region. Regions with live blocks are untouched. */
  clear(): void {
    for (let i = 0; i < this._regions.length; i++) {
      const region = this._regions[i];
      if (!region || region.used_bytes > 0) continue;
      this._regions[i] = null;
      this.stat.regions--;
      this.stat.reserved_bytes -= this.region_bytes;
      this.stat.idle_bytes -= this.region_bytes;
    }
  }
}
