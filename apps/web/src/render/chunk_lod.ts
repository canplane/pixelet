// @pxl/web/render/chunk_lod.ts

/**
 * The one real distance-to-camera measurement any chunk-shaped content uses
 * to pick a LOD tier. chunk_stage.tsx's terrain chunks and decor_layer.ts's
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

import { Frustum, Matrix4, Sphere, Vector3 } from "three";
import type { Camera, Group } from "three";
import type { f64 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
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

export type InitialChunkView = { in_frustum: boolean; distance2: f64; lod_step: LodStep };

/**
 * A one-shot, synchronous camera-relative estimate for a chunk that has no
 * mounted group yet to measure through chunk_distance_to_camera — computed
 * straight from `canonical_origin` (the chunk's own anchor-space origin,
 * page_base + its own offset) against `map_origin`
 * (AnchorRegistry.cur_map_origin), the exact "before any group exists"
 * hand-rolled math this file's own top doc comment warns drifts from the
 * live floating-origin frame. That drift is acceptable here specifically
 * because this is a ONE-SHOT seed, not the ongoing per-frame measurement:
 * without it, a ChunkLodControl starts with distance2 at a true sentinel
 * (Infinity) and in_frustum guessed some other way, so an entire batch of
 * controls mounted in the same frame (a big pan/zoom) has nothing but
 * registration order to rank by until each one gets its own first real
 * recheck() — which, for a large batch under the scheduler's own recheck
 * budget, can take several frames, during which "closest to the camera
 * gets attention first" silently degrades to "mounted first gets attention
 * first" instead. A same-frame batch that starts with a real (if
 * approximate) camera-distance instead ranks correctly from frame one;
 * the first genuine recheck() (using a real mounted group's matrixWorld)
 * then corrects any drift, exactly like the in_frustum guess already does.
 * z=0 (no real elevation yet) is the same approximation the ongoing
 * frustum test already accepts for a chunk that hasn't measured its own
 * true bounds yet.
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
