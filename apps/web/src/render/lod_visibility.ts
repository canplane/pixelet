// @pxl/web/render/lod_visibility.ts

/**
 * Shared "which of a chunk's own primary/secondary LOD meshes should
 * actually draw right now" registry — used by both chunk_stage.tsx's own
 * cube content (its ChunkMesh) and decor (decor_layer.ts's own
 * DecorVariantMesh). Lives here, not in either of those two files, so
 * neither has to import the other's own mesh-tracking types:
 * chunk_stage.tsx already imports FROM decor_layer.ts one-directionally
 * (see that file's own top doc comment for why the reverse direction was
 * removed as a circular import, 2026-09-05) — decor_layer.ts importing a
 * registry type back out of chunk_stage.tsx would reopen exactly that.
 *
 * Cube content toggles per-vertex (`a_visible` attribute + fragment
 * discard — chunk_stage.tsx's own inject_chunk_shader): a chunk's built
 * cube mesh is reused across LOD transitions (bucket_cache_ref), so "is
 * this mesh's role currently the one that should draw" has to be settable
 * without touching the geometry itself, which a shared attribute update
 * does far more cheaply than a rebuild. Decor has no such reuse-across-
 * tiers concern (decor_layer.ts's own per-step build cache already hands
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

/**
 * How much of each role to draw, encoded so one float per vertex carries
 * both the amount and which side of the dither pattern this mesh takes.
 *
 *   0 .. 1   draw a fragment where the pattern is below this value.
 *            1 is everything, 0 is nothing — the old boolean, unchanged.
 *   2 .. 3   the same value minus 2, but inverted: draw where the pattern
 *            is at or above it.
 *
 * The inversion is what makes a pair complementary. Both meshes sample the
 * same pattern, so a fine mesh at 2 + f and a coarse one at f cover every
 * fragment exactly once between them — no gaps to see through and no
 * fragment paying for both.
 */
export type ChunkVisibilityDecision = { primary: number; secondary: number };

/** What share of its fragments an encoded value actually keeps. */
export function visibility_coverage(value: number): number {
  return value > 1.5 ? 1 - (value - 2) : value;
}

export function apply_visibility(registry: ChunkMeshRegistry, decision: ChunkVisibilityDecision): void {
  for (const mounted of registry.meshes) {
    const value = mounted.role === "primary" ? decision.primary : decision.secondary;
    if (mounted.kind === "cube") {
      if (mounted.built.visibility[0] === value) continue;
      mounted.built.visibility.fill(value);
      mounted.attribute.needsUpdate = true;
    } else {
      // Decor is whole meshes, not per-vertex selectors — there is nothing
      // to dither, so it takes whichever side of the transition it is mostly
      // on. See decor_layer.ts for why it never needed the finer grain.
      mounted.mesh.visible = visibility_coverage(value) > 0.5;
    }
  }
}
