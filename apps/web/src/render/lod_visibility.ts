// @pxl/web/render/lod_visibility.ts

/**
 * Which of a chunk's own primary/secondary LOD meshes should actually draw
 * right now.
 *
 * Cube content toggles per-vertex (`a_visible` attribute + fragment discard —
 * chunk_material.ts's own shader injection): a chunk's built cube mesh is
 * reused across LOD transitions (bucket_cache), so "is this mesh's role
 * currently the one that should draw" has to be settable without touching the
 * geometry itself, which a shared attribute update does far more cheaply than
 * a rebuild.
 *
 * Decor used to be the second kind here, toggling a whole mesh's own
 * `THREE.Object3D.visible`. It no longer has meshes of its own to toggle:
 * decor/field.ts owns one instanced mesh per variant and a chunk contributes
 * the instances that should be drawn right now, so a tier change is a
 * different contribution rather than a flag on a mesh that exists either way.
 * What survives of that here is `visibility_coverage`, which is how a chunk
 * reads one decision as "which tier is decor mostly on".
 */

import type * as THREE from "three";

import type { BuiltChunkMesh, ChunkLodRole } from "@/render/layer/pipeline/chunk_bucket.ts";

export type MountedLodMesh = { built: BuiltChunkMesh; role: ChunkLodRole; attribute: THREE.BufferAttribute };

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
    if (mounted.built.visibility[0] === value) continue;
    mounted.built.visibility.fill(value);
    mounted.attribute.needsUpdate = true;
  }
}
