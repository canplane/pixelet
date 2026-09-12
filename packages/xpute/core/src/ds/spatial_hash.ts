// @xpute/core/ds/spatial_hash.ts

/**
 * Broadphase index for sparse, moving points in 3D, rebuilt every frame and
 * allocating nothing while it does.
 *
 * The regime this is for is the one an octree is worst at: many points in a
 * small volume, moving. A tree prunes nothing when everything lands in two
 * leaves and charges traversal for the privilege, and a dense grid pays for
 * cells it never fills — which is fatal when the world has no bounds to
 * size a grid against. Hashing quantised positions is insensitive to both
 * density and emptiness, and re-insertion on movement is a bucket write.
 *
 * **Counting sort, not a map of buckets.** Zhyhallo (ICAART 2024) revisits
 * Teschner's 2003 algorithm and finds the bottleneck is RAM I/O rather than
 * CPU — so the layout outranks the algorithm, and more cores do not help.
 * The build is therefore three passes over flat integer arrays: count per
 * bucket, prefix-sum the starts, place each id with a bump. No `Map`, no
 * per-entity object, no array literal. That is also what makes a per-frame
 * rebuild free of garbage, which on this platform is the difference between
 * a frame and a collection pause.
 *
 * **A generation stamp replaces the clear.** Teschner's trick: each rebuild
 * increments a counter, and a bucket whose stamp is stale reads as empty
 * and is overwritten rather than being zeroed in a reset pass. The prefix
 * sum then walks only the buckets actually written this generation — kept
 * in `touched` — so the cost tracks the points present, never the table
 * size. This is Zhyhallo's chunked allocation reduced to its essential
 * part: never pay for the empty regions of an unbounded space.
 *
 * **Cell size is the caller's, and it is the query radius.** Teschner
 * measured cell size against primitive extent and found the optimum at a
 * ratio of exactly one, and that cell size matters more to performance than
 * table size. Here the thing being asked about is a query, so its extent is
 * the radius: one cell per radius keeps a query to 27 buckets in 3D and
 * keeps a point in exactly one. A different consumer with a different
 * radius wants a different index rather than a compromise in this one.
 *
 * **Broadphase, so the caller does the exact test.** `query` reports every
 * id whose *cell* the query box overlaps — no false negatives, and false
 * positives bounded by geometry at one cell. Positions are not stored here:
 * the caller already holds them and already needs them for whatever it does
 * with a hit, and every precedent draws the line in the same place.
 *
 * The cell coordinate of each item *is* stored, and that is not the same
 * thing. Without it a hash collision hands back points from anywhere in the
 * world, so the caller's rejection cost would scale with the hash's quality
 * and with how much world exists — which is exactly the property this
 * structure is chosen for not having. Three extra integer lanes buy an
 * error bound that is a statement about geometry instead of a hope about a
 * hash, and they remove repeats along with it: a bucket reached twice by a
 * wide box emits only the cell it was reached for.
 *
 * **Capacity is fixed and exceeding it drops rather than grows.** A
 * structure whose job is to make the live object count constant must not
 * itself be the thing that scales with the scene. Overflow is reported, not
 * absorbed.
 *
 * Portable by construction: five integer arrays and index arithmetic, which
 * is the same in Rust over `Vec<u32>` and under WASM over linear memory.
 * `Math.imul` is there because JavaScript multiplies in doubles and the
 * hash needs 32-bit wraparound; in any other language the operator is
 * already correct.
 */

import type { f64, i32, u32 } from "../kit/word.ts";
import { HEAP_ALLOC, type TypedAlloc } from "./typed_alloc.ts";

/**
 * Teschner's three large primes. Their only job is to decorrelate the axes
 * before the xor, so that a lattice of points does not collapse onto a few
 * buckets — any large primes do, and these are the published ones, kept so
 * a reader can match this against the paper.
 */
const P1 = 73856093;
const P2 = 19349663;
const P3 = 83492791;

/** 27 in 3D: the query box can straddle at most one cell boundary per axis
 * when the radius is the cell size, which is the sizing this is built for.
 * Larger radii sweep more and remain correct. */
export const NEIGHBOUR_CELLS_3D = 27;

export type SpatialHashStats = {
  /** Points inserted in the current generation. */
  count: u32;
  /** Insertions refused because capacity was reached. */
  dropped: u32;
  /** Buckets written in the current generation. */
  touched: u32;
};

export class SpatialHash {
  /** Multiply rather than shift: world coordinates are real-valued here, and
   * for a power-of-two cell the reciprocal is exact, so this is the shift
   * that Zhyhallo's integer form spells `>>`. */
  private readonly inv_cell: f64;

  private readonly capacity: u32;
  private readonly bucket_count: u32;

  /** Per bucket, valid only while `stamps[b] === generation`. */
  private readonly counts: Uint32Array;
  private readonly starts: Uint32Array;
  private readonly stamps: Uint32Array;

  /** Buckets written this generation, so the prefix sum never walks the
   * whole table. */
  private readonly touched: Uint32Array;
  private touched_len: u32 = 0;

  /** Parallel to insertion order: the bucket each inserted id hashed to, so
   * the placing pass does not re-derive it. */
  private readonly cells: Uint32Array;
  /** Parallel to insertion order: the caller's id and its cell. */
  private readonly ids: Uint32Array;
  private readonly ins_cx: Int32Array;
  private readonly ins_cy: Int32Array;
  private readonly ins_cz: Int32Array;

  /** The sorted output: ids grouped by bucket. */
  private readonly items: Uint32Array;
  /** Parallel to `items`: the cell each one actually occupies, so a bucket
   * shared by unrelated cells can be filtered down to the cell asked for. */
  private readonly item_cx: Int32Array;
  private readonly item_cy: Int32Array;
  private readonly item_cz: Int32Array;

  /** Bump cursor per bucket during the placing pass, folded back into
   * `starts` afterwards so `starts[b]` is the group's first index. */
  private readonly cursors: Uint32Array;

  private generation: u32 = 0;
  private count: u32 = 0;
  private dropped: u32 = 0;
  private built = false;

  /**
   * @param capacity      most points in one generation; further insertions are dropped
   * @param bucket_count  prime, and comfortably above `capacity` — Teschner
   *                      measured the curve flattening well before that, and
   *                      the cost of overshooting is memory rather than time
   * @param cell_size     the query radius this index will be asked about
   */
  constructor(capacity: u32, bucket_count: u32, cell_size: f64, alloc: TypedAlloc = HEAP_ALLOC) {
    this.capacity = capacity;
    this.bucket_count = bucket_count;
    this.inv_cell = 1 / cell_size;

    this.counts = alloc.take(Uint32Array, bucket_count);
    this.starts = alloc.take(Uint32Array, bucket_count);
    this.stamps = alloc.take(Uint32Array, bucket_count);
    this.cursors = alloc.take(Uint32Array, bucket_count);

    this.touched = alloc.take(Uint32Array, bucket_count);
    this.cells = alloc.take(Uint32Array, capacity);
    this.ids = alloc.take(Uint32Array, capacity);
    this.ins_cx = alloc.take(Int32Array, capacity);
    this.ins_cy = alloc.take(Int32Array, capacity);
    this.ins_cz = alloc.take(Int32Array, capacity);

    this.items = alloc.take(Uint32Array, capacity);
    this.item_cx = alloc.take(Int32Array, capacity);
    this.item_cy = alloc.take(Int32Array, capacity);
    this.item_cz = alloc.take(Int32Array, capacity);
  }

  /** Cell coordinate of a world coordinate on one axis. */
  cell_of(v: f64): i32 {
    return Math.floor(v * this.inv_cell);
  }

  /** Bucket for a cell coordinate triple. */
  private bucket(cx: i32, cy: i32, cz: i32): u32 {
    const h = (Math.imul(cx, P1) ^ Math.imul(cy, P2) ^ Math.imul(cz, P3)) >>> 0;
    return h % this.bucket_count;
  }

  /**
   * Opens a generation. Nothing is cleared: the stamp makes every bucket
   * from the previous generation read as empty on its next touch.
   *
   * The counter is `u32` and wraps at 2^32; a wrap would make stale buckets
   * read as current, so it resets the stamps once — every 4 billion
   * rebuilds, which at 120 Hz is a little over a year of continuous frames.
   */
  begin(): void {
    this.generation = (this.generation + 1) >>> 0;
    if (this.generation === 0) {
      this.stamps.fill(0);
      this.generation = 1;
    }
    this.touched_len = 0;
    this.count = 0;
    this.dropped = 0;
    this.built = false;
  }

  /** Records one point. Returns false when capacity is spent. */
  insert(id: u32, x: f64, y: f64, z: f64): boolean {
    if (this.count >= this.capacity) {
      this.dropped++;
      return false;
    }
    const cx = this.cell_of(x), cy = this.cell_of(y), cz = this.cell_of(z);
    const b = this.bucket(cx, cy, cz);

    if (this.stamps[b] !== this.generation) {
      this.stamps[b] = this.generation;
      this.counts[b] = 0;
      this.touched[this.touched_len++] = b;
    }
    this.counts[b]++;

    const n = this.count++;
    this.cells[n] = b;
    this.ids[n] = id;
    this.ins_cx[n] = cx;
    this.ins_cy[n] = cy;
    this.ins_cz[n] = cz;
    return true;
  }

  /**
   * Closes the generation: prefix-sum the starts over the touched buckets,
   * then place every recorded id.
   *
   * The bump cursors are a separate array rather than a mutated copy of
   * `starts`, because `starts` has to survive as the group offset for the
   * queries that follow.
   */
  build(): void {
    let offset: u32 = 0;
    for (let t: u32 = 0; t < this.touched_len; t++) {
      const b = this.touched[t];
      this.starts[b] = offset;
      this.cursors[b] = offset;
      offset += this.counts[b];
    }
    for (let n: u32 = 0; n < this.count; n++) {
      const b = this.cells[n];
      const at = this.cursors[b]++;
      this.items[at] = this.ids[n];
      this.item_cx[at] = this.ins_cx[n];
      this.item_cy[at] = this.ins_cy[n];
      this.item_cz[at] = this.ins_cz[n];
    }
    this.built = true;
  }

  /**
   * Every id whose cell overlaps the axis-aligned box of `radius` about a
   * point, written into `out`, returning how many.
   *
   * Broadphase: no false negatives, and false positives only from the cell
   * quantisation itself — the caller applies the exact test it was going to
   * apply anyway. No id appears twice. Stops at `out.length` and reports
   * the truncation through the return value, since a caller with a
   * fixed-size result buffer has already decided how many it can use.
   */
  query(x: f64, y: f64, z: f64, radius: f64, out: Uint32Array): u32 {
    if (!this.built) return 0;

    const x0 = this.cell_of(x - radius), x1 = this.cell_of(x + radius);
    const y0 = this.cell_of(y - radius), y1 = this.cell_of(y + radius);
    const z0 = this.cell_of(z - radius), z1 = this.cell_of(z + radius);

    let n: u32 = 0;
    for (let cz = z0; cz <= z1; cz++) {
      for (let cy = y0; cy <= y1; cy++) {
        for (let cx = x0; cx <= x1; cx++) {
          const b = this.bucket(cx, cy, cz);
          if (this.stamps[b] !== this.generation) continue;
          const start = this.starts[b];
          const end = start + this.counts[b];
          for (let i = start; i < end; i++) {
            // The bucket is shared with every other cell that hashes here,
            // and with itself when a wide box reaches it twice. Comparing
            // the cell is what turns "probably nearby" into "in the box, or
            // one quantisation step outside it".
            if (this.item_cx[i] !== cx || this.item_cy[i] !== cy || this.item_cz[i] !== cz) continue;
            if (n >= out.length) return n;
            out[n++] = this.items[i];
          }
        }
      }
    }
    return n;
  }

  stats(): SpatialHashStats {
    return { count: this.count, dropped: this.dropped, touched: this.touched_len };
  }
}
