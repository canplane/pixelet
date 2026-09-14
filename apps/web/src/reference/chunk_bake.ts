// @pxl/web/reference/chunk_bake.ts

/**
 * The TypeScript bake as the reference tests drive it: one bake context
 * over the core pools, and a chunk bucket built from column runs through
 * it — what chunk_node.ts did before the module baked every tier
 * (crates/pxl-core/src/bake/mesher.rs), kept so the tests can hold the
 * module's mesh to it byte for byte.
 */

import type { LodStep } from "@/module/world/lod.ts";
import { bake_chunk_mesh, BakeContext } from "@/reference/chunk_mesher.ts";
import type { ColumnRunSet } from "@/reference/voxel_scan.ts";
import { bucket_from_baked, type BuiltChunkBucket, type ChunkLodRole } from "@/render/mesh/chunk_bucket.ts";
import { CORE_POOL, FIXED_ALLOC } from "@/wasm/core_pool.ts";

export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/**
 * Builds one reusable GPU-ready LOD from `column_runs` — every column's
 * runs, scanned once from the live volume (`voxel_scan.ts#scan_all_column_runs`)
 * and safe to reuse across many calls after that volume is released.
 *
 * Returns with one reference already held, for whoever asked. Every further
 * owner takes its own via retain_built_mesh.
 */
export function build_chunk_bucket(
  column_runs: ColumnRunSet,
  lod_step: LodStep,
  role: ChunkLodRole,
): BuiltChunkBucket {
  return bucket_from_baked(bake_chunk_mesh(CORE_BAKE, column_runs, lod_step), lod_step, role);
}
