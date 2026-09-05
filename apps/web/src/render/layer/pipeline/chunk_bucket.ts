// @pxl/web/render/layer/pipeline/chunk_bucket.ts

/** One renderer-owned L2 64x64 XY column. */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { LodStep } from "@/render/lod.ts";
import type { MeshRun } from "./voxel_scan.ts";
import { bake_chunk_mesh, type BakedChunkMesh } from "./chunk_mesher.ts";

export type ChunkOffsets = {
  key: string;
  /** Page-local chunk origin in canonical units. */
  base_x: f64;
  base_y: f64;
};

export type ChunkLodRole = "primary" | "secondary";

export type BuiltChunkMesh = BakedChunkMesh & {
  /** Mutable per-vertex selector; both LODs stay resident only inside a transition band. */
  visibility: Float32Array;
};

export type BuiltChunkBucket = {
  lod_step: LodStep;
  role: ChunkLodRole;
  /** One merged mesh for every cube in the chunk — terrain and solid
   * objects alike (see chunk_mesher.ts's own doc comment for why they no
   * longer stay split into two meshes). */
  cubes: BuiltChunkMesh;
};

/** Renderer-agnostic local AABB for every vertex retained by one batch. */
export type ChunkBounds = {
  min_x: f64;
  min_y: f64;
  min_z: f64;
  max_x: f64;
  max_y: f64;
  max_z: f64;
};

type BoundsAcc = { min_x: f64; min_y: f64; min_z: f64; max_x: f64; max_y: f64; max_z: f64; found: boolean };

/** Direct min/max scan over baked vertex positions — simpler than the old
 * instance-matrix decomposition this replaced, since a baked vertex's
 * position already IS a real point on the mesh's own surface, not a center
 * +scale pair a caller had to reconstruct a box from. */
function accumulate_mesh_bounds(mesh: BakedChunkMesh, acc: BoundsAcc): void {
  const position = mesh.position;
  if (mesh.vertex_cnt === 0) return;
  acc.found = true;
  for (let i = 0; i < mesh.vertex_cnt; i++) {
    const o = i * 3;
    const x = position[o], y = position[o + 1], z = position[o + 2];
    acc.min_x = Math.min(acc.min_x, x);
    acc.min_y = Math.min(acc.min_y, y);
    acc.min_z = Math.min(acc.min_z, z);
    acc.max_x = Math.max(acc.max_x, x);
    acc.max_y = Math.max(acc.max_y, y);
    acc.max_z = Math.max(acc.max_z, z);
  }
}

/**
 * Derives an exact AABB from one mesh's own baked vertex positions without
 * importing a renderer. Callers with a live mesh (chunk_stage.tsx) use this
 * instead of THREE's InstancedMesh.computeBoundingBox/computeBoundingSphere
 * equivalents to skip walking every vertex through a generic transform —
 * here there's no transform to walk at all, the positions are already real.
 */
export function mesh_bounds(mesh: BakedChunkMesh): ChunkBounds | null {
  const acc: BoundsAcc = { min_x: Infinity, min_y: Infinity, min_z: Infinity, max_x: -Infinity, max_y: -Infinity, max_z: -Infinity, found: false };
  accumulate_mesh_bounds(mesh, acc);
  return acc.found ? { min_x: acc.min_x, min_y: acc.min_y, min_z: acc.min_z, max_x: acc.max_x, max_y: acc.max_y, max_z: acc.max_z } : null;
}

/** Derives an exact AABB across every bucket's own merged cube mesh. */
export function chunk_bounds(buckets: readonly BuiltChunkBucket[]): ChunkBounds | null {
  const acc: BoundsAcc = { min_x: Infinity, min_y: Infinity, min_z: Infinity, max_x: -Infinity, max_y: -Infinity, max_z: -Infinity, found: false };
  for (const bucket of buckets) accumulate_mesh_bounds(bucket.cubes, acc);
  return acc.found ? { min_x: acc.min_x, min_y: acc.min_y, min_z: acc.min_z, max_x: acc.max_x, max_y: acc.max_y, max_z: acc.max_z } : null;
}

function with_visibility(built: BakedChunkMesh, role: ChunkLodRole): BuiltChunkMesh {
  const visibility = new Float32Array(built.vertex_cnt);
  visibility.fill(role === "primary" ? 1 : 0);
  return { ...built, visibility };
}

/**
 * Builds one reusable GPU-ready LOD from `column_runs` — every column's
 * runs, scanned once from the live volume (`voxel_scan.ts#scan_all_column_runs`)
 * and safe to reuse across many calls after that volume is released.
 */
export function build_chunk_bucket(
  column_runs: readonly MeshRun[][],
  lod_step: LodStep,
  role: ChunkLodRole,
): BuiltChunkBucket {
  return { lod_step, role, cubes: with_visibility(bake_chunk_mesh(column_runs, lod_step), role) };
}

/** Builds one primary LOD and, only while blending, one adjacent LOD. */
export function build_chunk_buckets(
  column_runs: readonly MeshRun[][],
  primary_step: LodStep,
  secondary_step: LodStep | null,
): BuiltChunkBucket[] {
  const out: BuiltChunkBucket[] = [];
  out.push(build_chunk_bucket(column_runs, primary_step, "primary"));

  if (secondary_step !== null && secondary_step !== primary_step) {
    out.push(build_chunk_bucket(column_runs, secondary_step, "secondary"));
  }
  return out.sort((a, b) => a.lod_step - b.lod_step);
}
