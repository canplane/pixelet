// @pxl/web/render/lod_visibility.ts

/**
 * Shared "which of a chunk's own primary/secondary LOD meshes should
 * actually draw right now" registry — used by both chunk_stage.tsx's own
 * cube content (its ChunkMesh) and decor (decor_layer.tsx's own
 * DecorVariantMesh). Lives here, not in either of those two files, so
 * neither has to import the other's own mesh-tracking types:
 * chunk_stage.tsx already imports FROM decor_layer.tsx one-directionally
 * (see that file's own top doc comment for why the reverse direction was
 * removed as a circular import, 2026-09-05) — decor_layer.tsx importing a
 * registry type back out of chunk_stage.tsx would reopen exactly that.
 *
 * Cube content toggles per-vertex (`a_visible` attribute + fragment
 * discard — chunk_stage.tsx's own inject_chunk_shader): a chunk's built
 * cube mesh is reused across LOD transitions (bucket_cache_ref), so "is
 * this mesh's role currently the one that should draw" has to be settable
 * without touching the geometry itself, which a shared attribute update
 * does far more cheaply than a rebuild. Decor has no such reuse-across-
 * tiers concern (decor_layer.tsx's own per-step build cache already hands
 * back a fresh, already-baked mesh per LOD step) and no per-vertex
 * attribute infrastructure to piggyback on either, so it toggles the
 * coarser `THREE.Object3D.visible` instead — same observable effect (this
 * mesh currently draws or it doesn't), a cheaper mechanism for content that
 * never needed per-vertex granularity in the first place.
 */

import type * as THREE from "three";

import type { BuiltChunkMesh, ChunkLodRole } from "@/render/layer/pipeline/chunk_bucket.ts";

export type MountedLodMesh =
  | { kind: "cube"; built: BuiltChunkMesh; role: ChunkLodRole; attribute: THREE.BufferAttribute }
  | { kind: "decor"; mesh: THREE.Mesh; role: ChunkLodRole };

export type ChunkMeshRegistry = { meshes: Set<MountedLodMesh> };
export type ChunkVisibilityDecision = { primary: boolean; secondary: boolean };

export function apply_visibility(registry: ChunkMeshRegistry, decision: ChunkVisibilityDecision): void {
  for (const mounted of registry.meshes) {
    const want_visible = mounted.role === "primary" ? decision.primary : decision.secondary;
    if (mounted.kind === "cube") {
      const value = want_visible ? 1 : 0;
      if (mounted.built.visibility[0] === value) continue;
      mounted.built.visibility.fill(value);
      mounted.attribute.needsUpdate = true;
    } else {
      mounted.mesh.visible = want_visible;
    }
  }
}
