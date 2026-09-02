// @pxl/web/render/chunk_lod.ts

/**
 * The one real distance-to-camera measurement any chunk-shaped content uses
 * to pick a LOD tier. chunk_stage.tsx's terrain chunks and decor_layer.tsx's
 * decor chunks are equal callers of this, not producer/consumer — both
 * mount a group under the same AnchorSpace, at a position ultimately owned
 * by world.tsx's own frame system (rotation + a continuously-translating
 * local-world frame, on top of the coarser anchor rebase baseline
 * AnchorRegistry.cur_map_origin only jumps for occasionally). camera.position
 * lives in the plain scene root's space, not any one of those nested frames,
 * so reconstructing a chunk's "real" position by redoing that frame math by
 * hand (as chunk_stage.tsx's own get_initial_chunk_view does, once, before
 * any group exists to read yet) silently drops the live translation frame's
 * own offset — measured error against a real mounted group's own
 * `matrixWorld` (which Three.js keeps correctly composed through every
 * nested frame automatically) is the only way this stays exact.
 */

import type { Camera, Frustum, Group, Matrix4, Vector3 } from "three";
import type { f64 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/** Same slack chunk_stage.tsx's own Chunk.recheck() uses for its
 * fallback-bounds sphere test — a chunk's contents (terrain or decor alike)
 * never reach further from its own center than this in practice, so it's
 * generous enough to never falsely cull a chunk that's actually still
 * partly on screen. */
export const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;

/**
 * World-space distance from `camera` to the point (local_x, local_y,
 * local_z) expressed in `group`'s own local space — e.g. a chunk's nominal
 * center within its own chunk-offset group (or, for a group with no such
 * offset of its own, a chunk's local position within that group instead).
 * `scratch` is mutated and reused so this can run every frame for every
 * visible chunk with no allocation.
 */
export function chunk_distance_to_camera(
  group: Group,
  local_x: f64,
  local_y: f64,
  local_z: f64,
  camera: Camera,
  scratch: Vector3,
): f64 {
  scratch.set(local_x, local_y, local_z).applyMatrix4(group.matrixWorld);
  return scratch.distanceTo(camera.position);
}

/**
 * Refreshes `frustum` to `camera`'s current one — the same
 * projectionMatrix*matrixWorldInverse incantation chunk_stage.tsx's own
 * ChunkStage and get_initial_chunk_view each spelled out independently
 * before this existed. `scratch_matrix` is mutated and reused so this, like
 * chunk_distance_to_camera, costs no allocation per call.
 */
export function update_frustum(frustum: Frustum, camera: Camera, scratch_matrix: Matrix4): void {
  scratch_matrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  frustum.setFromProjectionMatrix(scratch_matrix);
}
