// @pxl/web/modules/world/billboard/frustum_budget.ts

/**
 * Shared visibility budget for world-anchored billboards (tier 3 — see
 * discussions/20260907-world-rendering-tiers.md). A consumer with more
 * candidates than it can afford to mount (POI today; a future multiplayer
 * chat-bubble-per-character feature would hit the same shape) calls
 * select_visible_billboards once per selection tick:
 *
 * 1. Real camera-frustum membership — distance/FAR_END fading
 *    (distance_fade.ts) still owns within-frustum fade; this is a coarser
 *    pre-filter.
 * 2. If still over max_count: a stable hash of each candidate's own id, not
 *    distance-nearest (would always favor the same cluster) and not a fresh
 *    per-call Math.random() (would flicker frame to frame).
 */

import * as THREE from "three";
import type { Camera } from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { update_frustum } from "@/render/chunk_lod.ts";

export type BillboardCandidate<T> = {
  item: T;
  id: string;
  /** Position relative to local_world_frame's own origin — see
   * AnchorNode's own local-offset math (frame/coord.ts). */
  local_x: f64;
  local_y: f64;
  local_z: f64;
};

const $scratch_matrix = new THREE.Matrix4();
const $frustum = new THREE.Frustum();
const $local_pos = new THREE.Vector3();

function point_in_frustum(local_x: f64, local_y: f64, local_z: f64, local_world_frame: THREE.Object3D, frustum: THREE.Frustum): boolean {
  $local_pos.set(local_x, local_y, local_z);
  local_world_frame.localToWorld($local_pos);
  return frustum.containsPoint($local_pos);
}

// Same FNV-1a-into-[0,1) shape as chunk_stage.tsx's own module-private
// blend_hash — duplicated rather than imported to avoid coupling to that
// file's own internals.
function stable_hash(key: string): f64 {
  let hash = 2166136261;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) / 0x1_0000_0000;
}

/** `local_world_frame`: the live group whose matrixWorld carries every
 * transform from `local_x/y/z` to true world space (see
 * frame/context.tsx's useFrameRefs, `local_world_frame_ref.current`). */
export function select_visible_billboards<T>(
  candidates: readonly BillboardCandidate<T>[],
  local_world_frame: THREE.Object3D,
  camera: Camera,
  max_count: u32,
): T[] {
  update_frustum($frustum, camera, $scratch_matrix);

  const in_frustum: { item: T; id: string }[] = [];
  for (const c of candidates) {
    if (point_in_frustum(c.local_x, c.local_y, c.local_z, local_world_frame, $frustum)) {
      in_frustum.push({ item: c.item, id: c.id });
    }
  }

  if (in_frustum.length <= max_count) return in_frustum.map((c) => c.item);

  in_frustum.sort((a, b) => stable_hash(a.id) - stable_hash(b.id));
  return in_frustum.slice(0, max_count).map((c) => c.item);
}
