// @pxl/web/module/world/sched/priority.ts

/**
 * What goes first: one ordering, shared by everything the tick schedules.
 *
 * The rule is distance from **where the camera is going**, not from where
 * it is. Terrain fetch and the POI index already worked that way — both key
 * on `map_center`, which is the target the view is heading to. Chunks were
 * the one scheduler measuring from the camera as drawn, which lags that
 * target by the frame's damping (frame/use_view_frame.ts), so during a
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
 *
 * Distance is aged by how long an item has waited, within one class of work.
 * The order itself is the module's (crates/pxl-core/src/world/sched.rs); the lead
 * point it orders by is worked out here, from the frames the glue holds.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Mat4, Vec3 } from "@xpute/core/kit/mat4.ts";

/**
 * The camera position the view is heading to, in the view frame.
 *
 * `target_local` is the target in the root frame's own coordinates and
 * `root` is that frame's matrix, so the result lands in the same space as
 * every chunk centre — rotation included. The camera looks at the view
 * frame's origin, which is where the target will sit once the frame has
 * caught up, so the target's current position *is* the offset the camera
 * still has to travel.
 */
export function lead_point(out: Vec3, eye: Vec3, target_local_x: f64, target_local_y: f64, target_local_z: f64, root: Mat4): Vec3 {
  const e = root;
  out.x = eye.x + e[0] * target_local_x + e[4] * target_local_y + e[8] * target_local_z + e[12];
  out.y = eye.y + e[1] * target_local_x + e[5] * target_local_y + e[9] * target_local_z + e[13];
  out.z = eye.z + e[2] * target_local_x + e[6] * target_local_y + e[10] * target_local_z + e[14];
  return out;
}
