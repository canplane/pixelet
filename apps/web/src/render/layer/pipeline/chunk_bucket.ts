// @pxl/web/render/layer/pipeline/chunk_bucket.ts

/** One renderer-owned L2 64x64 XY column. */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { LodStep } from "@/render/lod.ts";
import type { MeshRun } from "./voxel_scan.ts";
import { build_chunk_instances, type BuiltChunk } from "./chunk_mesher.ts";

export type ChunkOffsets = {
  key: string;
  /** Page-local chunk origin in canonical units. */
  base_x: f64;
  base_y: f64;
};

export type ChunkLodRole = "primary" | "secondary";

export type BuiltChunkMesh = BuiltChunk & {
  /** Mutable selector; both LODs stay resident only inside a transition band. */
  visibility: Float32Array;
};

export type BuiltChunkBucket = {
  lod_step: LodStep;
  role: ChunkLodRole;
  terrain: BuiltChunkMesh;
  objects: BuiltChunkMesh;
};

/** Renderer-agnostic local AABB for every instance retained by one batch. */
export type ChunkBounds = {
  min_x: f64;
  min_y: f64;
  min_z: f64;
  max_x: f64;
  max_y: f64;
  max_z: f64;
};

type BoundsAcc = { min_x: f64; min_y: f64; min_z: f64; max_x: f64; max_y: f64; max_z: f64; found: boolean };

function accumulate_mesh_bounds(mesh: BuiltChunk, acc: BoundsAcc): void {
  const matrices = mesh.matrices;
  for (let instance = 0; instance < mesh.cnt; instance++) {
    const off = instance * 16;
    const half_x = (Math.abs(matrices[off]) + Math.abs(matrices[off + 4]) + Math.abs(matrices[off + 8])) * 0.5;
    const half_y = (Math.abs(matrices[off + 1]) + Math.abs(matrices[off + 5]) + Math.abs(matrices[off + 9])) * 0.5;
    const half_z = (Math.abs(matrices[off + 2]) + Math.abs(matrices[off + 6]) + Math.abs(matrices[off + 10])) * 0.5;
    const center_x = matrices[off + 12];
    const center_y = matrices[off + 13];
    const center_z = matrices[off + 14];
    acc.min_x = Math.min(acc.min_x, center_x - half_x);
    acc.min_y = Math.min(acc.min_y, center_y - half_y);
    acc.min_z = Math.min(acc.min_z, center_z - half_z);
    acc.max_x = Math.max(acc.max_x, center_x + half_x);
    acc.max_y = Math.max(acc.max_y, center_y + half_y);
    acc.max_z = Math.max(acc.max_z, center_z + half_z);
    acc.found = true;
  }
}

/**
 * Derives an exact AABB from one mesh's column-major instance matrices
 * without importing a renderer — the axis-aligned-only fast path
 * `write_scale_translation` guarantees, versus three.js's `InstancedMesh
 * .computeBoundingBox`/`computeBoundingSphere`, which walk every instance
 * through a generic (rotation-capable) per-vertex transform. Callers with a
 * live mesh (chunk_stage.tsx) use this instead of those to skip that cost.
 */
export function mesh_bounds(mesh: BuiltChunk): ChunkBounds | null {
  const acc: BoundsAcc = { min_x: Infinity, min_y: Infinity, min_z: Infinity, max_x: -Infinity, max_y: -Infinity, max_z: -Infinity, found: false };
  accumulate_mesh_bounds(mesh, acc);
  return acc.found ? { min_x: acc.min_x, min_y: acc.min_y, min_z: acc.min_z, max_x: acc.max_x, max_y: acc.max_y, max_z: acc.max_z } : null;
}

/**
 * Derives an exact AABB from the column-major instance matrices without
 * importing a renderer. This remains correct if boxes gain rotation later.
 */
export function chunk_bounds(buckets: readonly BuiltChunkBucket[]): ChunkBounds | null {
  const acc: BoundsAcc = { min_x: Infinity, min_y: Infinity, min_z: Infinity, max_x: -Infinity, max_y: -Infinity, max_z: -Infinity, found: false };
  for (const bucket of buckets) {
    accumulate_mesh_bounds(bucket.terrain, acc);
    accumulate_mesh_bounds(bucket.objects, acc);
  }
  return acc.found ? { min_x: acc.min_x, min_y: acc.min_y, min_z: acc.min_z, max_x: acc.max_x, max_y: acc.max_y, max_z: acc.max_z } : null;
}

function with_visibility(built: BuiltChunk, role: ChunkLodRole): BuiltChunkMesh {
  const visibility = new Float32Array(built.cnt);
  visibility.fill(role === "primary" ? 1 : 0);
  return { ...built, visibility };
}

function bucket_from_built(
  lod_step: LodStep,
  role: ChunkLodRole,
  built: ReturnType<typeof build_chunk_instances>,
): BuiltChunkBucket {
  return {
    lod_step,
    role,
    terrain: with_visibility(built.terrain, role),
    objects: with_visibility(built.objects, role),
  };
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
  return bucket_from_built(lod_step, role, build_chunk_instances(column_runs, lod_step));
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
