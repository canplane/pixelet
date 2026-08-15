// @pxl/web/render/layer/pipeline/chunk_bucket.ts

/** One renderer-owned L2 64x64 XY column. */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { LODStep } from "@/render/lod.ts";
import type { ChunkVolume } from "./volume.ts";
import { build_chunk_instances, build_chunk_instances_from_surface, build_chunk_surface, type BuiltChunk, type ChunkSurface } from "./chunk_mesher.ts";

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
  lod_step: LODStep;
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

/**
 * Derives an exact AABB from the column-major instance matrices without
 * importing a renderer. This remains correct if boxes gain rotation later.
 */
export function chunk_bounds(buckets: readonly BuiltChunkBucket[]): ChunkBounds | null {
  let min_x = Infinity;
  let min_y = Infinity;
  let min_z = Infinity;
  let max_x = -Infinity;
  let max_y = -Infinity;
  let max_z = -Infinity;
  let found = false;

  for (const bucket of buckets) {
    for (const mesh of [bucket.terrain, bucket.objects]) {
      const matrices = mesh.matrices;
      for (let instance = 0; instance < mesh.cnt; instance++) {
        const off = instance * 16;
        const half_x = (Math.abs(matrices[off]) + Math.abs(matrices[off + 4]) + Math.abs(matrices[off + 8])) * 0.5;
        const half_y = (Math.abs(matrices[off + 1]) + Math.abs(matrices[off + 5]) + Math.abs(matrices[off + 9])) * 0.5;
        const half_z = (Math.abs(matrices[off + 2]) + Math.abs(matrices[off + 6]) + Math.abs(matrices[off + 10])) * 0.5;
        const center_x = matrices[off + 12];
        const center_y = matrices[off + 13];
        const center_z = matrices[off + 14];
        min_x = Math.min(min_x, center_x - half_x);
        min_y = Math.min(min_y, center_y - half_y);
        min_z = Math.min(min_z, center_z - half_z);
        max_x = Math.max(max_x, center_x + half_x);
        max_y = Math.max(max_y, center_y + half_y);
        max_z = Math.max(max_z, center_z + half_z);
        found = true;
      }
    }
  }

  return found ? { min_x, min_y, min_z, max_x, max_y, max_z } : null;
}

function with_visibility(built: BuiltChunk, role: ChunkLodRole): BuiltChunkMesh {
  const visibility = new Float32Array(built.cnt);
  visibility.fill(role === "primary" ? 1 : 0);
  return { ...built, visibility };
}

function bucket_from_built(
  lod_step: LODStep,
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

/** Builds one reusable GPU-ready LOD from the dense field. */
export function build_chunk_bucket(
  volume: ChunkVolume,
  lod_step: LODStep,
  role: ChunkLodRole,
  surface?: ChunkSurface,
): BuiltChunkBucket {
  return bucket_from_built(lod_step, role, build_chunk_instances(volume, lod_step, surface));
}

/** Builds one reusable coarse LOD without recompositing the world layers. */
export function build_chunk_bucket_from_surface(
  surface: ChunkSurface,
  lod_step: LODStep,
  role: ChunkLodRole,
): BuiltChunkBucket {
  return bucket_from_built(lod_step, role, build_chunk_instances_from_surface(surface, lod_step));
}

/** Builds one primary LOD and, only while blending, one adjacent LOD. */
export function build_chunk_buckets(
  volume: ChunkVolume,
  primary_step: LODStep,
  secondary_step: LODStep | null,
): BuiltChunkBucket[] {
  const surface = primary_step > 1 || (secondary_step !== null && secondary_step > 1) ? build_chunk_surface(volume) : undefined;
  const out: BuiltChunkBucket[] = [];
  out.push(build_chunk_bucket(volume, primary_step, "primary", surface));

  if (secondary_step !== null && secondary_step !== primary_step) {
    out.push(build_chunk_bucket(volume, secondary_step, "secondary", surface));
  }
  return out.sort((a, b) => a.lod_step - b.lod_step);
}
