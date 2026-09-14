// @pxl/web/reference/chunk_bake.ts

/**
 * The TypeScript bake as the reference tests drive it: one bake context
 * over the core pools, a tier baked from column runs through it, and the
 * bounds of what was baked — what the client did before the module baked
 * every tier (crates/pxl-core/src/bake/mesher.rs), kept so the tests can
 * hold the module's mesh to it byte for byte.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { LodStep } from "@/module/world/lod.ts";
import { bake_chunk_mesh, BakeContext, type BakedChunkMesh } from "@/reference/chunk_mesher.ts";
import type { ColumnRunSet } from "@/reference/voxel_scan.ts";
import { CORE_POOL, FIXED_ALLOC } from "@/wasm/core_pool.ts";

export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/** One tier baked from `column_runs` — every column's runs, scanned once
 * from a volume (`voxel_scan.ts#scan_all_column_runs`) and reusable after
 * that volume is released — as the TypeScript baked it. */
export function bake_tier(column_runs: ColumnRunSet, lod_step: LodStep): BakedChunkMesh {
  return bake_chunk_mesh(CORE_BAKE, column_runs, lod_step);
}

/** The exact AABB of every vertex across `meshes`, or null with none: what
 * the module's frustum test is held to. */
export function mesh_bounds(meshes: readonly BakedChunkMesh[]): { min_x: f64; min_y: f64; min_z: f64; max_x: f64; max_y: f64; max_z: f64 } | null {
  let found = false;
  let min_x = Infinity, min_y = Infinity, min_z = Infinity, max_x = -Infinity, max_y = -Infinity, max_z = -Infinity;
  for (const mesh of meshes) {
    const p = mesh.position;
    for (let i = 0; i < mesh.vertex_cnt; i++) {
      found = true;
      const o = i * 3;
      min_x = Math.min(min_x, p[o]);
      min_y = Math.min(min_y, p[o + 1]);
      min_z = Math.min(min_z, p[o + 2]);
      max_x = Math.max(max_x, p[o]);
      max_y = Math.max(max_y, p[o + 1]);
      max_z = Math.max(max_z, p[o + 2]);
    }
  }
  return found ? { min_x, min_y, min_z, max_x, max_y, max_z } : null;
}
