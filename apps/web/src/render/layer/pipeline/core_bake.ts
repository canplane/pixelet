// @pxl/web/render/layer/pipeline/core_bake.ts

/**
 * The main thread's bake context: the mesher's scratch and counters over the
 * core pool, which is where every mesh the renderer draws lives. Kept out of
 * chunk_mesher.ts so that importing the mesher reserves nothing — a worker
 * builds its own context over its own memory, and only the main thread
 * imports this one.
 */

import { CORE_POOL, FIXED_ALLOC } from "./core_pool.ts";
import { BakeContext, type BakedChunkMesh } from "./chunk_mesher.ts";

export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/** Hands one baked mesh's block back to the core pool. Every view into it is
 * dead afterwards. */
export function release_chunk_mesh(mesh: BakedChunkMesh): void {
  CORE_POOL.release(mesh.block);
}
