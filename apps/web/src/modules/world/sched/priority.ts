// @pxl/web/modules/world/sched/priority.ts

/**
 * What goes first: one ordering, shared by everything the tick schedules.
 *
 * The rule is distance from **where the camera is going**, not from where
 * it is. Terrain fetch and the POI index already worked that way — both key
 * on `map_center`, which is the target the view is heading to. Chunks were
 * the one scheduler measuring from the camera as drawn, which lags that
 * target by the frame's damping (use_frame_translation.ts), so during a
 * pan they filled in behind the view rather than ahead of it, and during a
 * search jump they kept polishing the place being left.
 *
 * The lag is the lookahead, and it needs no tuning: the shown frame chases
 * the target with a critically damped step, so the gap between them is the
 * travel still to come — about a tenth of a second of motion on a steady
 * drag (POS_DAMP = 10), and the whole distance on a jump. At rest the two
 * coincide and this orders exactly as plain camera distance did.
 *
 * Only the order changes. Which LOD tier a chunk shows is still decided by
 * its distance from the camera as drawn, because that is what is on screen.
 */

import type * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";

/**
 * The camera position the view is heading to, in world space.
 *
 * `target_local` is the target in the local world frame's own coordinates
 * and `local_frame` is that frame's world matrix, so the result lands in the
 * same space as every chunk centre — rotation included. The camera looks at
 * the world origin, which is where the target will sit once the frame has
 * caught up, so the target's current world position *is* the offset the
 * camera still has to travel.
 *
 * Pass the frame's matrix as it stands, not a freshly updated one: chunk
 * centres are read through the same one-frame-old matrices, so the two agree,
 * and updating the frame's world matrix here would walk every chunk under it.
 */
export function lead_point(
  out: THREE.Vector3,
  camera_position: THREE.Vector3,
  target_local_x: f64,
  target_local_y: f64,
  target_local_z: f64,
  local_frame: THREE.Matrix4,
): THREE.Vector3 {
  const e = local_frame.elements;
  const x = e[0] * target_local_x + e[4] * target_local_y + e[8] * target_local_z + e[12];
  const y = e[1] * target_local_x + e[5] * target_local_y + e[9] * target_local_z + e[13];
  const z = e[2] * target_local_x + e[6] * target_local_y + e[10] * target_local_z + e[14];
  return out.set(camera_position.x + x, camera_position.y + y, camera_position.z + z);
}

export type Prioritised = {
  in_frustum: boolean;
  /** Squared distance from `lead_point`. */
  lead_distance2: f64;
  /** Holding a slot every other item is waiting on — goes first, whatever
   * else is true of it. */
  holds_slot?: boolean;
};

/** The ordering. Slot holders, then what is on screen, then what is nearest
 * the place the view is heading. */
export function by_priority(a: Prioritised, b: Prioritised): number {
  return Number(b.holds_slot ?? false) - Number(a.holds_slot ?? false) ||
    Number(b.in_frustum) - Number(a.in_frustum) ||
    a.lead_distance2 - b.lead_distance2;
}
