// @pxl/web/modules/world/billboard/frustum_budget.ts

/**
 * Shared visibility budget for world-anchored billboards (tier 3 — see
 * discussions/20260907-world-rendering-tiers.md). A consumer with more
 * candidates than it can afford to mount (POI today; a future multiplayer
 * chat-bubble-per-character feature would hit the same shape) calls
 * select_visible_billboards once per selection tick:
 *
 * 1. Real camera-frustum membership, tested against a sphere around the
 *    anchor rather than the bare anchor point — distance/FAR_END fading
 *    (distance_fade.ts) still owns within-frustum fade; this is a coarser
 *    pre-filter.
 *
 *    The sphere matters at an oblique camera. A billboard is drawn *above*
 *    its anchor on screen, so an anchor that has just dropped below the
 *    bottom edge still has its pill in full view — and at an oblique polar
 *    the bottom edge is where the *nearest* anchors are (the camera sits on
 *    the target's -Y side and looks down at it; see camera/orbit.ts). Point
 *    membership therefore deleted the closest badges first while distant
 *    ones near the horizon stayed, and zooming in made it worse by pushing
 *    more anchors past that edge. Looking straight down the artifact
 *    disappears, because there the anchor and its pill share a screen
 *    position.
 * 2. If still over max_count: nearest first, with a stable hash of each
 *    candidate's own id breaking ties.
 *
 * The ranking used to be the hash alone, chosen so the survivors would not
 * flicker frame to frame the way Math.random() would. It achieved that and
 * still produced the worst artifact this file has had: the cap is a cliff.
 * At or under max_count every candidate draws; one candidate over, the whole
 * set is ranked by a value unrelated to where anything is and cut. So the
 * smallest zoom-out that pulled an 81st POI into the frustum could delete a
 * badge in the middle of the screen while keeping one at the edge, with
 * nothing in front of either.
 *
 * Distance restores monotonicity: widening the view can only append farther
 * candidates, so the ones already on screen keep their slots and what falls
 * off the end is the farthest — which is also the smallest and the least
 * legible. The original objection to distance was that it always favours the
 * same cluster, and that is true and is the point: the near cluster is the
 * one being looked at.
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
const $sphere = new THREE.Sphere();
const $camera_pos = new THREE.Vector3();

/**
 * How far around its anchor a billboard can reach, as a fraction of its own
 * distance from the camera.
 *
 * A fraction rather than a world length because use_distance_fade holds a
 * billboard at a roughly constant *screen* size, so its world extent grows
 * with distance. This is that angular size with room to spare — generous on
 * purpose, since the cost of keeping one badge too many is one sprite and
 * the cost of dropping one too early is the bug above.
 */
const ANCHOR_REACH: f64 = 0.08;

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
  // getWorldPosition, not camera.position: the two agree today because the
  // camera hangs off the scene root, but the frustum above is world-space
  // and a distance measured from a local offset would rank by the wrong
  // origin the moment anything parents the camera.
  camera.getWorldPosition($camera_pos);

  const in_frustum: { item: T; id: string; d2: f64 }[] = [];
  for (const c of candidates) {
    // localToWorld gives the point the frustum test needs and the point the
    // distance is measured from, so it is done once rather than twice.
    $local_pos.set(c.local_x, c.local_y, c.local_z);
    local_world_frame.localToWorld($local_pos);
    const d2 = $local_pos.distanceToSquared($camera_pos);
    $sphere.set($local_pos, Math.sqrt(d2) * ANCHOR_REACH);
    if (!$frustum.intersectsSphere($sphere)) continue;
    in_frustum.push({ item: c.item, id: c.id, d2 });
  }

  if (in_frustum.length <= max_count) return in_frustum.map((c) => c.item);

  // Squared distance: only the order matters, so the square root does not.
  // The hash breaks ties rather than deciding anything, which keeps two
  // candidates at the same distance from swapping places frame to frame.
  in_frustum.sort((a, b) => a.d2 - b.d2 || stable_hash(a.id) - stable_hash(b.id));
  return in_frustum.slice(0, max_count).map((c) => c.item);
}
