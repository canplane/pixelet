// @pxl/web/render/layer/pipeline/mesh_pool.ts

/**
 * The one buddy pool every per-chunk buffer comes from.
 *
 * Its own module because two producers share it and neither can import the
 * other: chunk_mesher.ts bakes the vertex block, voxel_scan.ts packs the
 * column-run set, and the mesher already imports the scanner. Sharing one
 * pool rather than one each is deliberate — both are per-chunk, both are
 * freed when a chunk's tier changes, and buddy can only move bytes between
 * sizes that live in the same region.
 */

import { Pool } from "@xpute/core/ds/pool.ts";

/**
 * A chunk mesh's whole block, recycled across rebuilds.
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
export const MESH_POOL = new Pool({
  min_block_bytes: 1 << 12,
  region_bytes: 1 << 26,
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

export function mesh_pool_stat(): Readonly<typeof MESH_POOL.stat> {
  return MESH_POOL.stat;
}
