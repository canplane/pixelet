// @pxl/web/render/layer/pipeline/core_pool.ts

/**
 * The one buddy pool the core's data lives in, carved out of the core
 * memory below.
 *
 * It was the mesh pool, and is renamed for what it has become: chunk
 * meshes (chunk_mesher.ts), column-run sets (voxel_scan.ts) and resident
 * terrain pages (world/terrain/resident.ts) all borrow from it. One pool
 * rather than one per producer is deliberate — buddy can only move bytes
 * between sizes that live in the same region, and one memory is the point:
 * discussions/20260911-single-thread-core-and-when-to-spawn-a-worker.md.
 */

import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { type AddressedAlloc, buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";

/**
 * Region sizing.
 *
 * Region size is a balance between the largest real block and how much a
 * single live block can strand. A region is only reclaimed once it is
 * *fully* empty, so at 256MB one surviving chunk held a quarter-gigabyte
 * hostage: measured, 339MB live against 1502MB idle.
 *
 * 256MB was chosen when a dense urban chunk at LOD1 measured 67MB. Face
 * culling then dropped real terrain to 24% of the faces it used to emit, so
 * the largest block is around a quarter of what it was and the region can
 * shrink with it — `pool.stat.max_block_bytes` is the number to check this
 * against, and the report prints it. Anything above a region still works; it
 * just spills to its own buffer and is never reused.
 *
 * `poison` stays off even in development — it is a memset of every released
 * block, which would distort the very rebuild storm this pool exists to
 * measure. Turn it on here when hunting a view that outlived its release
 * (the one failure mode pooling introduces): the stale reader then sees NaN
 * geometry instead of a plausible neighbouring chunk.
 */
/**
 * The core's memory: one fixed-length `WebAssembly.Memory`, reserved up front
 * and never grown, that the pool carves its first regions out of.
 *
 * Fixed-length rather than resizable, and never grown, for three measured
 * reasons (discussions/20260911-single-thread-core-and-when-to-spawn-a-worker.md):
 * a view into a grown memory is detached, and every mesh span and run lane
 * is such a view; views into a resizable buffer read two and a half times
 * slower in V8; and WebGL and WebGPU reject them outright. A reservation is
 * committed lazily — untouched pages cost address space, not memory — so it
 * is sized for the pool's peak rather than to the byte. JavaScriptCore
 * nonetheless reports the whole reservation as the page's JavaScript
 * memory (a Safari heap timeline read it as one 512 MB ArrayBuffer), which
 * is why a phone gets half of it below.
 *
 * Eight regions, 512 MB. Four were sized for meshes alone and the ledger
 * already read 384 MB reserved at a load peak; terrain pages add about 4 MB
 * each, up to the 49 the cleanup radius keeps (terrain/config.ts). Past the
 * reservation the pool allocates plain regions exactly as it always has.
 *
 * Tried at half the size when the full one is refused, and null when that
 * fails too: a device that refuses both is exactly a device that should not
 * have one, and the pool then behaves as it did before this existed.
 */
const CORE_REGIONS = 8;
/** Half on a phone. Whatever the OS commits, JavaScriptCore reports the
 * whole reservation against the page, and the page's budget there is a
 * fraction of the desktop's; past it the pool hands out plain regions the
 * collector can take back. */
const HANDHELD_REGIONS = CORE_REGIONS / 2;
const REGION_BYTES = 1 << 26;
const WASM_PAGE = 1 << 16;

function reserve_core_memory(): WebAssembly.Memory | null {
  const first = HANDHELD ? HANDHELD_REGIONS : CORE_REGIONS;
  for (const regions of [first, first / 2]) {
    const pages = (regions * REGION_BYTES) / WASM_PAGE;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // too large for this device; try the smaller reservation
    }
  }
  return null;
}

export const CORE_MEMORY: WebAssembly.Memory | null = reserve_core_memory();

export const CORE_POOL = new BuddyAllocator({
  min_block_bytes: 1 << 12,
  region_bytes: REGION_BYTES,
  backing: CORE_MEMORY?.buffer,
  max_idle_regions: 1,
  poison: false,
  // On again. The precondition it was waiting for is met: a chunk mesh now
  // has exactly two owners — chunk_node.ts's LOD cache and the layout naming
  // it — and both are mutated synchronously by the node, in one call, with
  // the geometry attached and detached in the same step. There is no longer
  // a window in which a layout names meshes nothing holds, which is what
  // made recycling unsafe while React decided mount timing.
  //
  // If `[pool] double release` or `[chunk_bucket] retain of a released mesh`
  // ever appears again, that is a real ownership bug and not noise: turn
  // `poison` on to catch the stale reader at the read rather than the
  // release.
  bypass: false,
});

/** Typed lanes carved from the core pool — what a structure is handed so its
 * columns live in the core memory rather than on the heap. */
export const CORE_ALLOC: AddressedAlloc = buddy_alloc(CORE_POOL);

export function core_pool_stat(): Readonly<typeof CORE_POOL.stat> {
  return CORE_POOL.stat;
}
