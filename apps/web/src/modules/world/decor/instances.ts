// @pxl/web/modules/world/decor/instances.ts

/**
 * A chunk's decor instances for one variant, as lanes.
 *
 * What an instance is: a chunk-local position, a spin about Z, a scale, and
 * where it came from — a user-placed entry in objects/store.ts, or a
 * procedural scatter that no store names. That was an object per instance
 * (~22,000 live at a wide zoom, 2026-09-12) held twice: once in the chunk's
 * per-step cache and once in decor/field.ts's `source` array so a hit could
 * find its id. Now it is six typed lanes in core memory, and the id is not
 * stored at all: a placed entry is named by its band and packed position in
 * the store's own lanes, two u32s, and the string id is rebuilt from those
 * only when a hit needs it (objects/store.ts's entry_id_of).
 *
 * `hidden` stays as a flag lane rather than a separate list: field.ts still
 * routes hidden instances to a never-rendered mesh so hit testing keeps
 * seeing them, and reads the flag per instance as it writes.
 *
 * A set has owners, counted: the node's per-step cache and the layout that
 * is showing it die independently — the cache is reclaimed off screen
 * while the layout waits for its release to commit, and a source change
 * empties the cache while a deferred rebuild keeps the layout — so "the
 * cache dropped it" cannot mean "the lanes are free". The same `rc` the
 * baked meshes carry (chunk_bucket.ts), for the same reason.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { TypedAlloc } from "@xpute/core/ds/typed_alloc.ts";
import { Vector } from "@xpute/core/ds/arena.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

/** Sentinel in `band` for a procedural instance: nothing in the store names
 * it, so a hit on it resolves to no entry. */
export const NO_ENTRY: u32 = 0xffffffff;

/** Scratch grows from here and lives in the core pool like every lane. */
const LANE = { init_cap: 1024, alloc: CORE_ALLOC } as const;

/** The lanes; `len` instances are live. One of these per (chunk, step,
 * variant), allocated from the core pool and handed back on release. */
export type DecorInstances = {
  /** Owners still holding this set; the lanes go back to the pool at zero. */
  rc: u32;
  len: u32;
  x: Float32Array;
  y: Float32Array;
  z: Float32Array;
  rotation_z: Float32Array;
  scale: Float32Array;
  /** Bit 0: hidden (set_virtual_hide is suppressing it). */
  flags: Uint8Array;
  /** The store entry this instance is, as objects/store.ts's own (band,
   * packed position); NO_ENTRY in `band` for a procedural instance. */
  band: Uint32Array;
  pos: Uint32Array;
};

export const HIDDEN_FLAG: u32 = 1;

/**
 * The builder every producer appends into — module scratch, so building a
 * chunk's instances allocates nothing per instance. `take` copies the
 * result out into exact-sized lanes; `reset` starts the next variant.
 */
const s_x = new Vector(Float32Array, LANE);
const s_y = new Vector(Float32Array, LANE);
const s_z = new Vector(Float32Array, LANE);
const s_rot = new Vector(Float32Array, LANE);
const s_scale = new Vector(Float32Array, LANE);
const s_flags = new Vector(Uint8Array, LANE);
const s_band = new Vector(Uint32Array, LANE);
const s_pos = new Vector(Uint32Array, LANE);

export function instances_reset(): void {
  s_x.truncate(0);
  s_y.truncate(0);
  s_z.truncate(0);
  s_rot.truncate(0);
  s_scale.truncate(0);
  s_flags.truncate(0);
  s_band.truncate(0);
  s_pos.truncate(0);
}

export function instances_push(x: f64, y: f64, z: f64, rotation_z: f64, scale: f64, flags: u32, band: u32, pos: u32): void {
  s_x.push(x);
  s_y.push(y);
  s_z.push(z);
  s_rot.push(rotation_z);
  s_scale.push(scale);
  s_flags.push(flags);
  s_band.push(band);
  s_pos.push(pos);
}

/** Copies what was pushed since the last reset into exact-sized lanes. An
 * empty build returns null rather than eight empty allocations. */
export function instances_take(alloc: TypedAlloc = CORE_ALLOC): DecorInstances | null {
  const n = s_x.len;
  if (n === 0) return null;
  const out: DecorInstances = {
    rc: 1,
    len: n,
    x: alloc.take(Float32Array, n),
    y: alloc.take(Float32Array, n),
    z: alloc.take(Float32Array, n),
    rotation_z: alloc.take(Float32Array, n),
    scale: alloc.take(Float32Array, n),
    flags: alloc.take(Uint8Array, n),
    band: alloc.take(Uint32Array, n),
    pos: alloc.take(Uint32Array, n),
  };
  out.x.set(s_x.mem.subarray(0, n));
  out.y.set(s_y.mem.subarray(0, n));
  out.z.set(s_z.mem.subarray(0, n));
  out.rotation_z.set(s_rot.mem.subarray(0, n));
  out.scale.set(s_scale.mem.subarray(0, n));
  out.flags.set(s_flags.mem.subarray(0, n));
  out.band.set(s_band.mem.subarray(0, n));
  out.pos.set(s_pos.mem.subarray(0, n));
  return out;
}

export function instances_retain(inst: DecorInstances): void {
  if (inst.rc <= 0) {
    console.warn(`[decor/instances] retain of a released set (${inst.len} instances) — its lanes are no longer ours`);
    return;
  }
  inst.rc++;
}

/** Drops one owner; the last one out returns the lanes to the pool. */
export function instances_release(inst: DecorInstances, alloc: TypedAlloc = CORE_ALLOC): void {
  if (inst.rc <= 0) {
    console.warn(`[decor/instances] release of an already-freed set (${inst.len} instances) ignored`);
    return;
  }
  if (--inst.rc > 0) return;
  alloc.give(inst.x);
  alloc.give(inst.y);
  alloc.give(inst.z);
  alloc.give(inst.rotation_z);
  alloc.give(inst.scale);
  alloc.give(inst.flags);
  alloc.give(inst.band);
  alloc.give(inst.pos);
}
