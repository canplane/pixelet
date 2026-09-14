// @pxl/web/render/mesh/decor_block.ts

/**
 * A chunk's decor instances as lanes, and the block the module hands them
 * over in.
 *
 * What an instance is: a chunk-local position, a spin about Z, a scale, and
 * where it came from — a user-placed entry in object/store.ts, or a
 * procedural scatter that no store names. The id is not stored: a placed
 * entry is named by its band and packed position in the store's own lanes,
 * two u32s, and the string id is rebuilt from those only when a hit needs
 * it (object/store.ts's entry_id_of). `hidden` is a flag lane rather than
 * a separate list: the vertex shader sends a hidden instance outside the
 * clip volume (decor.wgsl) while the hit test keeps seeing it.
 *
 * The module builds a chunk's decor at one tier as one block of the mesh
 * section (crates/pxl-core/src/world/decor.rs): every group's lanes — a group per
 * (kind, variant) in the order the kinds were taught — read here as views.
 * The block is one owner-counted thing, the same `rc` the baked meshes
 * carry (chunk_bucket.ts) for the same reason: the node's per-step cache
 * and the layout showing it die independently.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";

import { CORE_POOL } from "@/wasm/core_pool.ts";

/** Sentinel in `band` for a procedural instance: nothing in the store names
 * it, so a hit on it resolves to no entry. */
export const NO_ENTRY: u32 = 0xffffffff;

/** The lanes; `len` instances are live. One of these per (chunk, step,
 * variant): a group's views into a block, or the reference build's own
 * lanes (reference/decor_instance.ts). */
export type DecorInstances = {
  /** Owners of the reference build's lanes; a block's group is owned
   * through the block. */
  rc: u32;
  len: u32;
  x: Float32Array;
  y: Float32Array;
  z: Float32Array;
  rotation_z: Float32Array;
  scale: Float32Array;
  /** Bit 0: hidden (set_virtual_hide is suppressing it). */
  flags: Uint8Array;
  /** The store entry this instance is, as object/store.ts's own (band,
   * packed position); NO_ENTRY in `band` for a procedural instance. */
  band: Uint32Array;
  pos: Uint32Array;
};

export const HIDDEN_FLAG: u32 = 1;

/**
 * A chunk's decor at one tier as the module builds it
 * (crates/pxl-core/src/world/decor.rs): one block of the mesh section holding
 * every group's lanes — a group per (kind, variant) in the order the kinds
 * were taught — read here as views. The block is one owner-counted thing;
 * its groups' lanes live and die with it.
 */
export type DecorBlock = {
  rc: u32;
  readonly block: Block;
  /** One per group; null where the group has no instance. */
  readonly groups: readonly (DecorInstances | null)[];
};

/** Reads the block's header and lays views over its lanes. The layout is
 * decor.rs's: the group count, a count per group, then per group five f32
 * lanes, the flags padded to a word, the bands and the positions. */
export function decor_block_carve(block: Block): DecorBlock {
  const words = new Uint32Array(block.buf, block.byte_offset, block.bytes >> 2);
  const group_cnt = words[0];
  const groups: (DecorInstances | null)[] = [];
  let k = 1 + group_cnt;
  for (let g = 0; g < group_cnt; g++) {
    const n = words[1 + g];
    if (n === 0) {
      groups.push(null);
      continue;
    }
    const at = block.byte_offset + k * 4;
    const lane = (i: u32) => new Float32Array(block.buf, at + i * n * 4, n);
    const padded = (n + 3) >> 2;
    groups.push({
      rc: 1,
      len: n,
      x: lane(0),
      y: lane(1),
      z: lane(2),
      rotation_z: lane(3),
      scale: lane(4),
      flags: new Uint8Array(block.buf, at + 5 * n * 4, n),
      band: new Uint32Array(block.buf, at + (5 * n + padded) * 4, n),
      pos: new Uint32Array(block.buf, at + (6 * n + padded) * 4, n),
    });
    k += 7 * n + padded;
  }
  return { rc: 1, block, groups };
}

export function decor_block_retain(d: DecorBlock): void {
  if (d.rc <= 0) {
    console.warn(`[decor/instances] retain of a released block (${d.groups.length} groups) — it is no longer ours`);
    return;
  }
  d.rc++;
}

/** Drops one owner; the last one out returns the block to the pool. */
export function decor_block_release(d: DecorBlock): void {
  if (d.rc <= 0) {
    console.warn(`[decor/instances] release of an already-freed block (${d.groups.length} groups) ignored`);
    return;
  }
  if (--d.rc > 0) return;
  CORE_POOL.release(d.block);
}
