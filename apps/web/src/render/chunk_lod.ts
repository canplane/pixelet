// @pxl/web/render/chunk_lod.ts

/**
 * The camera's frustum, and the one-shot view estimate a chunk starts from.
 *
 * The per-frame measurement — where every chunk is and whether it is on
 * screen — is sched/chunk_table.ts's `refresh`, which takes every chunk
 * centre through the anchor root's own composed `matrixWorld`, the frame
 * world.tsx's frame system (rotation, the continuously-translating local
 * world frame, the coarser AnchorRegistry.cur_map_origin rebase) expresses
 * every anchor group in. Redoing that frame math by hand, as the estimate
 * below does once before any group exists, drops the live translation
 * frame's offset, which is why it seeds and never measures.
 */

import { Frustum, Matrix4, Sphere, Vector3 } from "three";
import type { Camera } from "three";
import type { f64 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { get_lod_guide, type LodStep } from "@/render/lod.ts";

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/** Same slack chunk_stage.tsx's own Chunk.recheck() uses for its
 * fallback-bounds sphere test — a chunk's contents (terrain or decor alike)
 * never reach further from its own center than this in practice, so it's
 * generous enough to never falsely cull a chunk that's actually still
 * partly on screen. */
export const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;

/**
 * Refreshes `frustum` to `camera`'s current one — the same
 * projectionMatrix*matrixWorldInverse incantation chunk_stage.tsx's own
 * ChunkStage and get_initial_chunk_view each spelled out independently
 * before this existed. `scratch_matrix` is mutated and reused so this costs
 * no allocation per call.
 */
export function update_frustum(frustum: Frustum, camera: Camera, scratch_matrix: Matrix4): void {
  scratch_matrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  frustum.setFromProjectionMatrix(scratch_matrix);
}

export type InitialChunkView = { in_frustum: boolean; distance2: f64; lod_step: LodStep };

/**
 * A one-shot, synchronous camera-relative estimate for a chunk that has no
 * mounted group yet — computed straight from `canonical_origin` (the chunk's
 * own anchor-space origin, page_base + its own offset) against `map_origin`
 * (AnchorRegistry.cur_map_origin), the "before any group exists" math this
 * file's own top doc comment warns drifts from the live floating-origin
 * frame. Acceptable because it is a seed, not the per-frame measurement:
 * the scheduler's table (sched/chunk_table.ts) measures every chunk before
 * anything is ordered, and what this decides is only the tier a fresh
 * node's first recheck starts its hysteresis from. z=0 (no real elevation
 * yet) is the same approximation the frustum test accepts for a chunk that
 * has not measured its own bounds.
 */
export function estimate_initial_chunk_view(
  camera: Camera,
  map_origin: IVec,
  canonical_origin: IVec,
): InitialChunkView {
  const local_origin = ivec.sub(canonical_origin, map_origin);
  const center = new Vector3(
    ivec.to_real(ivec.X(local_origin)) + CHUNK_WORLD_SPAN * 0.5,
    ivec.to_real(ivec.Y(local_origin)) + CHUNK_WORLD_SPAN * 0.5,
    0,
  );
  const frustum = new Frustum();
  update_frustum(frustum, camera, new Matrix4());
  const sphere = new Sphere(center, APPROXIMATE_CHUNK_RADIUS);
  // Same "camera sits directly over/inside this chunk" fallback
  // chunk_stage.tsx's own Chunk.recheck() applies for its unbuilt-bounds
  // case — a low-angle frustum can otherwise rule out the one chunk
  // directly beneath the camera.
  const camera_projection_inside = Math.abs(camera.position.x - center.x) <= CHUNK_WORLD_SPAN * 0.5 &&
    Math.abs(camera.position.y - center.y) <= CHUNK_WORLD_SPAN * 0.5;
  const in_frustum = camera_projection_inside || frustum.intersectsSphere(sphere);
  const distance2 = center.distanceToSquared(camera.position);
  return {
    in_frustum,
    distance2,
    lod_step: in_frustum ? get_lod_guide(Math.sqrt(distance2), 64).lod_step : 64,
  };
}
