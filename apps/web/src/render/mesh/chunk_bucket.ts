// @pxl/web/render/mesh/chunk_bucket.ts

/** One renderer-owned L2 64x64 XY column. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { LodStep } from "@/module/world/lod.ts";
import { type BakedChunkMesh, carve_visibility } from "@/render/mesh/mesh_block.ts";
import { release_chunk_mesh } from "@/wasm/core_build.ts";

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
  /**
   * How many owners still hold this mesh. Two of them exist and die
   * independently: chunk_stage.tsx's per-chunk LOD cache, and the mounted
   * geometry that renders it. A cache eviction while the same tier is still
   * on screen is the normal case, not an edge one (recheck() prunes on
   * frustum exit; commit() replaces the layout a frame later), so "the cache
   * dropped it" cannot mean "the block is free" — hence a count rather than
   * a single owner. In Rust this is what `Rc<MeshBlock>` would be.
   */
  rc: u32;
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
  const visibility = carve_visibility(built);
  // Always a full write, never a top-up: the span comes from a recycled
  // block and arrives holding the previous borrower's selectors.
  visibility.fill(role === "primary" ? 1 : 0);
  return { ...built, visibility, rc: 1 };
}

/** Wraps a mesh the module baked (or, in the reference tests, one
 * reference/chunk_bake.ts baked) into the bucket a node holds. */
export function bucket_from_baked(baked: BakedChunkMesh, lod_step: LodStep, role: ChunkLodRole): BuiltChunkBucket {
  return { lod_step, role, cubes: with_visibility(baked, role) };
}

/**
 * Takes a reference. Retaining a mesh nobody holds is a bug, not a revival:
 * its block is back in the pool and very likely already handed to another
 * chunk, so the geometry built from it would render someone else's
 * vertices. Loud here rather than as a "double release" one step later,
 * which names the symptom instead of the moment.
 */
export function retain_built_mesh(mesh: BuiltChunkMesh): void {
  if (mesh.rc <= 0) {
    console.warn(`[chunk_bucket] retain of a released mesh (${mesh.vertex_cnt} verts) — its block is no longer ours`);
    return;
  }
  mesh.rc++;
}

/**
 * Drops one reference, returning the mesh's pooled block once the last owner
 * lets go. Every view into it — including any BufferAttribute still pointing
 * at one — is dead from that moment: the next chunk to bake gets those exact
 * bytes.
 */
export function release_built_mesh(mesh: BuiltChunkMesh): void {
  if (mesh.rc <= 0) {
    console.warn(`[chunk_bucket] release of an already-freed mesh (${mesh.vertex_cnt} verts) ignored`);
    return;
  }
  if (--mesh.rc > 0) return;
  release_chunk_mesh(mesh);
}
