// @pxl/web/modules/world/sched/chunk_table.ts

/**
 * Every chunk the scheduler knows, as lanes.
 *
 * What the scheduler reads and writes about a chunk every frame — where it
 * is, whether it is on screen, how far from the camera and from where the
 * view is heading, what it is waiting for and since when, what it holds —
 * used to be fields on ~2,500 ChunkNode objects gathered into a Set, and
 * every pass was a walk over those objects. Here it is one slot per chunk
 * in typed lanes from the core pool, and a pass is a loop over a lane.
 *
 * A slot is dense: removing one moves the last slot into its place, so the
 * handle kept per slot (the ChunkNode, which is where the scene objects and
 * the work in flight still live) is told its new index. The handles are the
 * glue between these numbers and three.js; nothing here imports three.
 *
 * Positions are kept in the frame every chunk shares. A chunk's origin is an
 * absolute lattice value, and the anchor groups it hangs under are pure
 * translations from the map origin, so its centre in the anchor root's own
 * space is `base - origin + box centre`, and one matrix — the root's — takes
 * every centre to world space. That is what `refresh` does for all slots at
 * once, instead of each node reading its own group's composed matrix.
 */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;
/** Same slack render/chunk_lod.ts allows a chunk that has not measured its
 * own bounds: its contents never reach further from its centre than this. */
const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;

/** Bits of `flags`. */
export const IN_FRUSTUM: u32 = 1;
/** Nothing of this chunk is on screen yet: a hole to fill rather than a
 * chunk asking to be sharper. The two are different classes of work
 * (priority.ts). */
export const EMPTY: u32 = 2;
export const PENDING: u32 = 4;
/** Holding the scan slot (voxel_scan.ts) or the bake slot (chunk_mesher.ts).
 * Nothing else can use that slot until it finishes, so it goes first. */
export const HOLDS_SLOT: u32 = 8;
export const BLEND_ACTIVE: u32 = 16;
/** Showing a tier that is not the one wanted. */
export const SETTLING: u32 = 32;
/** IN_FRUSTUM changed in the last `refresh`; the stage mirrors it onto the
 * scene node and clears it. */
export const VISIBILITY_CHANGED: u32 = 64;

/** The lanes priority.ts orders by — the table, or a test's own few. */
export type PriorityLanes = {
  flags: Uint8Array;
  lead_distance2: Float64Array;
  pending_since: Float64Array;
};

const LANE = { init_cap: 4096, alloc: CORE_ALLOC } as const;

export class ChunkTable<T extends { slot: u32 }> {
  private _len: u32 = 0;

  /** Slot by chunk base, for whoever asks whether a chunk is live. */
  private readonly by_base = new IndexedMap(I64_KEYS, 4096, CORE_ALLOC);

  private readonly _flags = new Vector(Uint8Array, LANE);
  /** Absolute lattice origin: the packed word, and in units per axis. */
  private readonly _base = new Vector(BigUint64Array, LANE);
  private readonly _base_x = new Vector(Float64Array, LANE);
  private readonly _base_y = new Vector(Float64Array, LANE);
  /** Bounding sphere in the chunk's own space: the centre of its measured
   * box and half its diagonal, or the nominal centre and the approximate
   * radius until a build has measured one. */
  private readonly _bc_x = new Vector(Float64Array, LANE);
  private readonly _bc_y = new Vector(Float64Array, LANE);
  private readonly _bc_z = new Vector(Float64Array, LANE);
  private readonly _radius = new Vector(Float64Array, LANE);
  /** World-space centre, written by `refresh`. */
  private readonly _cx = new Vector(Float64Array, LANE);
  private readonly _cy = new Vector(Float64Array, LANE);
  private readonly _cz = new Vector(Float64Array, LANE);
  /** Squared distance from the camera; picks the tier. */
  private readonly _distance2 = new Vector(Float64Array, LANE);
  /** Squared distance from where the camera is heading; orders the work. */
  private readonly _lead_distance2 = new Vector(Float64Array, LANE);
  /** When the work now pending was asked for, in ms; -1 while not pending. */
  private readonly _pending_since = new Vector(Float64Array, LANE);
  /** The request the pending work is for (chunk_node.ts's request codes). */
  private readonly _request = new Vector(Int32Array, LANE);
  /** Bytes of built work the chunk keeps, on or off screen. */
  private readonly _retained = new Vector(Float64Array, LANE);

  /** The node behind each slot. */
  readonly handles: T[] = [];

  /**
   * Bytes every slot retains, split by the side of the frustum it is on:
   * `off` is what a reclaim pass can act on, `on` is the working set. Kept
   * as the lanes change so nothing walks them to find out.
   */
  retained_on: f64 = 0;
  retained_off: f64 = 0;

  get len(): u32 {
    return this._len;
  }

  get flags(): Uint8Array {
    return this._flags.mem;
  }
  get distance2(): Float64Array {
    return this._distance2.mem;
  }
  get lead_distance2(): Float64Array {
    return this._lead_distance2.mem;
  }
  get pending_since(): Float64Array {
    return this._pending_since.mem;
  }
  get request(): Int32Array {
    return this._request.mem;
  }
  get retained(): Float64Array {
    return this._retained.mem;
  }

  // ---- slots ----

  /** The slot of the chunk whose absolute base is `base`, or -1. */
  find(base: IVec): i32 {
    const pos = this.by_base.find(base);
    return pos < 0 ? -1 : this._by_base_slot.mem[pos];
  }
  private readonly _by_base_slot = new Vector(Uint32Array, LANE);

  /** Registers a chunk by its absolute lattice base. */
  add(handle: T, base: IVec): u32 {
    const slot = this._len++;
    handle.slot = slot;
    this.handles[slot] = handle;
    this._by_base_slot.set(this.by_base.insert(base), slot);
    this._base.set(slot, base);
    this._flags.set(slot, 0);
    this._base_x.set(slot, ivec.to_real(ivec.X(base)));
    this._base_y.set(slot, ivec.to_real(ivec.Y(base)));
    this._bc_x.set(slot, CHUNK_WORLD_SPAN * 0.5);
    this._bc_y.set(slot, CHUNK_WORLD_SPAN * 0.5);
    this._bc_z.set(slot, 0);
    this._radius.set(slot, APPROXIMATE_CHUNK_RADIUS);
    this._cx.set(slot, 0);
    this._cy.set(slot, 0);
    this._cz.set(slot, 0);
    this._distance2.set(slot, Infinity);
    this._lead_distance2.set(slot, Infinity);
    this._pending_since.set(slot, -1);
    this._request.set(slot, 0);
    this._retained.set(slot, 0);
    return slot;
  }

  /** Frees a slot; the last slot moves into it and its handle learns so. */
  remove(slot: u32): void {
    this.account(slot, -this.retained[slot]);
    const last = --this._len;
    // The base index: drop this chunk's entry, and point the moved chunk's
    // entry at its new slot.
    {
      const pos = this.by_base.find(this.base_of(slot));
      const from = this.by_base.remove_at(pos);
      if (from !== pos) this._by_base_slot.mem[pos] = this._by_base_slot.mem[from];
      this._by_base_slot.truncate(from);
    }
    if (slot !== last) {
      const moved = this.handles[last];
      moved.slot = slot;
      this.handles[slot] = moved;
      this._by_base_slot.mem[this.by_base.find(this.base_of(last))] = slot;
      this.flags[slot] = this.flags[last];
      this._base.mem[slot] = this._base.mem[last];
      this._base_x.mem[slot] = this._base_x.mem[last];
      this._base_y.mem[slot] = this._base_y.mem[last];
      this._bc_x.mem[slot] = this._bc_x.mem[last];
      this._bc_y.mem[slot] = this._bc_y.mem[last];
      this._bc_z.mem[slot] = this._bc_z.mem[last];
      this._radius.mem[slot] = this._radius.mem[last];
      this._cx.mem[slot] = this._cx.mem[last];
      this._cy.mem[slot] = this._cy.mem[last];
      this._cz.mem[slot] = this._cz.mem[last];
      this.distance2[slot] = this.distance2[last];
      this.lead_distance2[slot] = this.lead_distance2[last];
      this.pending_since[slot] = this.pending_since[last];
      this.request[slot] = this.request[last];
      this.retained[slot] = this.retained[last];
    }
    this.handles.length = last;
    this._flags.truncate(last);
    this._base.truncate(last);
    this._base_x.truncate(last);
    this._base_y.truncate(last);
    this._bc_x.truncate(last);
    this._bc_y.truncate(last);
    this._bc_z.truncate(last);
    this._radius.truncate(last);
    this._cx.truncate(last);
    this._cy.truncate(last);
    this._cz.truncate(last);
    this._distance2.truncate(last);
    this._lead_distance2.truncate(last);
    this._pending_since.truncate(last);
    this._request.truncate(last);
    this._retained.truncate(last);
  }

  /** The absolute base a slot was registered with. */
  base_of(slot: u32): IVec {
    return this._base.mem[slot];
  }

  // ---- per-slot writes ----

  has(slot: u32, flag: u32): boolean {
    return (this.flags[slot] & flag) !== 0;
  }

  set_flag(slot: u32, flag: u32, on: boolean): void {
    if (on) this.flags[slot] |= flag;
    else this.flags[slot] &= ~flag;
  }

  /** The chunk's bounding sphere in its own space, from a measured box. */
  set_bounds(slot: u32, cx: f64, cy: f64, cz: f64, radius: f64): void {
    this._bc_x.mem[slot] = cx;
    this._bc_y.mem[slot] = cy;
    this._bc_z.mem[slot] = cz;
    this._radius.mem[slot] = radius;
  }

  /**
   * The one place PENDING is written, so the time it became pending is never
   * out of step with it. The clock restarts when the request changes, not
   * only when pending goes false to true: an off-screen chunk stays pending
   * for as long as its release is queued, and keying on the flag alone let a
   * chunk come back on screen already holding those seconds
   * (discussions/20260912-what-to-render-when.md).
   */
  set_pending(slot: u32, pending: boolean, request: u32, now: f64): void {
    if (pending && (!this.has(slot, PENDING) || request !== this.request[slot])) this.pending_since[slot] = now;
    if (!pending) this.pending_since[slot] = -1;
    this.request[slot] = request;
    this.set_flag(slot, PENDING, pending);
  }

  /** Adds `delta` bytes to the slot and to whichever side of the frustum it
   * is on. */
  account(slot: u32, delta: f64): void {
    this.retained[slot] += delta;
    if (this.has(slot, IN_FRUSTUM)) this.retained_on += delta;
    else this.retained_off += delta;
  }

  // ---- the per-frame pass ----

  /**
   * Where every chunk is and whether it is on screen, from one matrix.
   *
   * `m` is the anchor root's world matrix (sixteen elements, column-major);
   * `origin_*` is the map origin in units, so `base - origin` is a chunk's
   * position in the root's space; `planes` is the frustum as six planes of
   * (nx, ny, nz, constant). One visibility rule for every chunk, a bounding
   * sphere: it never culls anything a transformed box would have kept, and
   * costs six plane dots instead of eight transformed corners. The camera
   * standing over a chunk keeps it visible whatever the planes say: a
   * low-angle frustum can otherwise rule out the one chunk beneath it.
   *
   * Sets VISIBILITY_CHANGED where IN_FRUSTUM moved, and moves the slot's
   * retained bytes to the side it is now on.
   */
  refresh(
    m: ArrayLike<number>,
    origin_x: f64,
    origin_y: f64,
    origin_z: f64,
    planes: Float64Array,
    cam_x: f64,
    cam_y: f64,
    cam_z: f64,
    lead_x: f64,
    lead_y: f64,
    lead_z: f64,
  ): void {
    const n = this._len;
    const flags = this.flags;
    const base_x = this._base_x.mem;
    const base_y = this._base_y.mem;
    const bc_x = this._bc_x.mem;
    const bc_y = this._bc_y.mem;
    const bc_z = this._bc_z.mem;
    const radius = this._radius.mem;
    const cx = this._cx.mem;
    const cy = this._cy.mem;
    const cz = this._cz.mem;
    const distance2 = this.distance2;
    const lead_distance2 = this.lead_distance2;
    const retained = this.retained;
    const half = CHUNK_WORLD_SPAN * 0.5;
    const m0 = m[0], m1 = m[1], m2 = m[2], m4 = m[4], m5 = m[5], m6 = m[6], m8 = m[8], m9 = m[9], m10 = m[10], m12 = m[12], m13 = m[13], m14 = m[14];

    for (let i = 0; i < n; i++) {
      const lx = base_x[i] - origin_x + bc_x[i];
      const ly = base_y[i] - origin_y + bc_y[i];
      const lz = bc_z[i] - origin_z;
      const x = m0 * lx + m4 * ly + m8 * lz + m12;
      const y = m1 * lx + m5 * ly + m9 * lz + m13;
      const z = m2 * lx + m6 * ly + m10 * lz + m14;
      cx[i] = x;
      cy[i] = y;
      cz[i] = z;
      distance2[i] = (x - cam_x) ** 2 + (y - cam_y) ** 2 + (z - cam_z) ** 2;
      lead_distance2[i] = (x - lead_x) ** 2 + (y - lead_y) ** 2 + (z - lead_z) ** 2;

      let visible = Math.abs(cam_x - x) <= half && Math.abs(cam_y - y) <= half;
      if (!visible) {
        visible = true;
        const r = radius[i];
        for (let p = 0; p < 24; p += 4) {
          if (planes[p] * x + planes[p + 1] * y + planes[p + 2] * z + planes[p + 3] < -r) {
            visible = false;
            break;
          }
        }
      }

      const was = (flags[i] & IN_FRUSTUM) !== 0;
      if (visible !== was) {
        const bytes = retained[i];
        if (visible) {
          this.retained_off -= bytes;
          this.retained_on += bytes;
          flags[i] |= IN_FRUSTUM | VISIBILITY_CHANGED;
        } else {
          this.retained_on -= bytes;
          this.retained_off += bytes;
          flags[i] = (flags[i] & ~IN_FRUSTUM) | VISIBILITY_CHANGED;
        }
      }
    }
  }

  /** How many slots have every bit of `flag`. */
  count(flag: u32): u32 {
    const flags = this.flags;
    let n = 0;
    for (let i = 0; i < this._len; i++) if ((flags[i] & flag) === flag) n++;
    return n;
  }
}
