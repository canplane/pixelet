// @pxl/web/modules/world/terrain/resident.ts

/**
 * A terrain page, resident in the core memory.
 *
 * A fetched page decodes into views over the response's own ArrayBuffer, so
 * every resident page used to be a 4 MB buffer of its own that only the
 * collector could reclaim. Here the decoded arrays are copied into the core
 * pool (render/layer/pipeline/core_pool.ts) and the response is dropped:
 * one memory holds the core's data, which is what the big/little decision in
 * discussions/20260911-single-thread-core-and-when-to-spawn-a-worker.md is
 * for.
 *
 * Per array, not per page. Buddy blocks are powers of two, and a whole page
 * (4 × 1 MiB channels plus a ~9 KB elevation grid) would round up to 8 MiB.
 * Each channel is exactly one 1 MiB block at PAGE_CELL_SPAN 1024, so nothing
 * is wasted but the elevation grid's rounding.
 *
 * **Release is quarantined.** Readers of a page are the chunks in its render
 * radius; an evicted page is past the cleanup radius, one ring further out,
 * so none should remain. The case that can still hold one is a jump: the
 * page is evicted in the same pass that moves the view, and its chunks leave
 * in the React commit after. A released block is only overwritten when
 * something acquires it again, so holding the release back for a second
 * covers that with room to spare, at the price of a second's worth of pages
 * staying borrowed.
 */

import type { Block } from "@xpute/core/ds/buddy.ts";
import type { Ground, GroundChannelKey, Terrain } from "@pxl/shared/bake/format.ts";

import { CORE_POOL } from "@/render/layer/pipeline/core_pool.ts";

export const QUARANTINE_MS = 1000;

/** The blocks each resident page owns. The shared ocean page is not in here:
 * every open-ocean page aliases one decode, so no single page may free it. */
const owned = new WeakMap<Terrain, Block[]>();

function copy_u8(src: Uint8Array, blocks: Block[]): Uint8Array<ArrayBuffer> {
  const block = CORE_POOL.acquire(src.byteLength);
  blocks.push(block);
  const view = new Uint8Array(block.buf, block.byte_offset, src.length);
  view.set(src);
  return view;
}

/**
 * Copies a decoded page into the core pool. `own` is false for a page that
 * is shared rather than held by one tile — it is copied once and never
 * released.
 */
export function into_core(src: Terrain, own: boolean = true): Terrain {
  const blocks: Block[] = [];
  const ele_block = CORE_POOL.acquire(src.ele.byteLength);
  blocks.push(ele_block);
  const ele = new Int16Array(ele_block.buf, ele_block.byte_offset, src.ele.length);
  ele.set(src.ele);

  const ground = {} as Ground;
  for (const key of Object.keys(src.ground) as GroundChannelKey[]) ground[key] = copy_u8(src.ground[key], blocks);

  const terrain: Terrain = { ele, ground };
  if (own) owned.set(terrain, blocks);
  return terrain;
}

/** Hands a page's blocks back once the quarantine has passed. A page it does
 * not own — the shared ocean page — is left alone. */
export function release_terrain(terrain: Terrain, quarantine_ms: number = QUARANTINE_MS): void {
  const blocks = owned.get(terrain);
  if (!blocks) return;
  owned.delete(terrain);
  setTimeout(() => {
    for (const block of blocks) CORE_POOL.release(block);
  }, quarantine_ms);
}
