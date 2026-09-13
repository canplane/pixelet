// @pxl/web/module/world/sched/chunk_table.ts

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
import { Lanes } from "@xpute/core/ds/lanes.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

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

const LANE = { init_cap: 4096, alloc: FIXED_ALLOC } as const;

/** One row per chunk: absolute lattice origin (the packed word and its two
 * axes in units), the bounding sphere in the chunk's own space (measured box
 * centre and half diagonal, or the nominal ones until a build measures),
 * the world-space centre `refresh` writes, the two squared distances (from
 * the camera, which picks the tier; from where the camera is heading, which
 * orders the work), when the pending work was asked for (-1 while not
 * pending), which request it is (chunk_node.ts's codes), and the bytes of
 * built work the chunk keeps. */
const SLOT_LANES = {
  flags: Uint8Array,
  base: BigUint64Array,
  base_x: Float64Array,
  base_y: Float64Array,
  bc_x: Float64Array,
  bc_y: Float64Array,
  bc_z: Float64Array,
  radius: Float64Array,
  cx: Float64Array,
  cy: Float64Array,
  cz: Float64Array,
  distance2: Float64Array,
  lead_distance2: Float64Array,
  pending_since: Float64Array,
  request: Int32Array,
  retained: Float64Array,
} as const;

export class ChunkTable<T extends { slot: u32 }> {
  /** Slot by chunk base, for whoever asks whether a chunk is live. */
  private readonly by_base = new IndexedMap(I64_KEYS, 4096, FIXED_ALLOC);
  /** Which slot the base at each index position names. */
  private readonly by_base_slot = new Vector(Uint32Array, LANE);

  private readonly slots = new Lanes(SLOT_LANES, LANE);

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
    return this.slots.len;
  }

  get flags(): Uint8Array {
    return this.slots.mem.flags;
  }
  get distance2(): Float64Array {
    return this.slots.mem.distance2;
  }
  get lead_distance2(): Float64Array {
    return this.slots.mem.lead_distance2;
  }
  get pending_since(): Float64Array {
    return this.slots.mem.pending_since;
  }
  get request(): Int32Array {
    return this.slots.mem.request;
  }
  get retained(): Float64Array {
    return this.slots.mem.retained;
  }

  // ---- slots ----

  /** The slot of the chunk whose absolute base is `base`, or -1. */
  find(base: IVec): i32 {
    const pos = this.by_base.find(base);
    return pos < 0 ? -1 : this.by_base_slot.mem[pos];
  }

  /** Registers a chunk by its absolute lattice base. */
  add(handle: T, base: IVec): u32 {
    const slot = this.slots.push();
    const m = this.slots.mem;
    handle.slot = slot;
    this.handles[slot] = handle;
    this.by_base_slot.set(this.by_base.insert(base), slot);
    m.base[slot] = base;
    m.flags[slot] = 0;
    m.base_x[slot] = ivec.to_real(ivec.X(base));
    m.base_y[slot] = ivec.to_real(ivec.Y(base));
    m.bc_x[slot] = CHUNK_WORLD_SPAN * 0.5;
    m.bc_y[slot] = CHUNK_WORLD_SPAN * 0.5;
    m.bc_z[slot] = 0;
    m.radius[slot] = APPROXIMATE_CHUNK_RADIUS;
    m.cx[slot] = 0;
    m.cy[slot] = 0;
    m.cz[slot] = 0;
    m.distance2[slot] = Infinity;
    m.lead_distance2[slot] = Infinity;
    m.pending_since[slot] = -1;
    m.request[slot] = 0;
    m.retained[slot] = 0;
    return slot;
  }

  /** Frees a slot; the last slot moves into it and its handle learns so. */
  remove(slot: u32): void {
    this.account(slot, -this.retained[slot]);
    // The base index: drop this chunk's entry, and point the moved chunk's
    // entry at its new slot.
    {
      const pos = this.by_base.find(this.base_of(slot));
      const from = this.by_base.remove_at(pos);
      if (from !== pos) this.by_base_slot.mem[pos] = this.by_base_slot.mem[from];
      this.by_base_slot.truncate(from);
    }
    const last = this.slots.len - 1;
    if (slot !== last) {
      const moved = this.handles[last];
      moved.slot = slot;
      this.handles[slot] = moved;
      this.by_base_slot.mem[this.by_base.find(this.base_of(last))] = slot;
    }
    this.slots.swap_remove(slot);
    this.handles.length = last;
  }

  /** The absolute base a slot was registered with. */
  base_of(slot: u32): IVec {
    return this.slots.mem.base[slot];
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
    const m = this.slots.mem;
    m.bc_x[slot] = cx;
    m.bc_y[slot] = cy;
    m.bc_z[slot] = cz;
    m.radius[slot] = radius;
  }

  /**
   * The one place PENDING is written, so the time it became pending is never
   * out of step with it. The clock restarts when the request changes, not
   * only when pending goes false to true: an off-screen chunk stays pending
   * for as long as its release is queued, and keying on the flag alone let a
   * chunk come back on screen already holding those seconds
   * (issue/20260912-what-to-render-when.md).
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
    const n = this.slots.len;
    const { flags, base_x, base_y, bc_x, bc_y, bc_z, radius, cx, cy, cz, distance2, lead_distance2, retained } = this.slots.mem;
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
    for (let i = 0; i < this.slots.len; i++) if ((flags[i] & flag) === flag) n++;
    return n;
  }
}
