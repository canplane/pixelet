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

import type { Camera, Group, Vector3 } from "three";
import type { f64 } from "@xpute/core/kit/word.ts";

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
