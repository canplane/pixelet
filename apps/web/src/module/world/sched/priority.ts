// @pxl/web/module/world/sched/priority.ts

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
 *
 * Distance is aged by how long an item has waited, within one class of work
 * — see AGE_HALF_MS.
 */

import type * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

import { EMPTY, HOLDS_SLOT, IN_FRUSTUM, type PriorityLanes } from "./chunk_table.ts";

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

/**
 * What an item is waiting for decides what it competes against. Four
 * classes, not one key with terms added to it: a chunk with nothing on screen
 * and a chunk that would merely like to be sharper are not making the same
 * request, and letting a wait trade one against the other is how the middle
 * of the screen ended up behind the far ring during a zoom-out
 * (issue/20260912-what-to-render-when.md). The classes are bits of the
 * table's flag lane (chunk_table.ts): HOLDS_SLOT, IN_FRUSTUM, EMPTY.
 */

/**
 * Ageing, the textbook cure for starvation under a static priority.
 *
 * Distance alone let a chunk that was always a little farther from the lead
 * than whatever kept arriving wait for as long as the view kept moving: one
 * on-screen chunk waited sixteen seconds in a zoomed-out pan. Every half-life
 * an item waits halves its effective distance, so one k times farther than a
 * fresh arrival overtakes it after log2(k) half-lives.
 *
 * The half-life has to be long against the time the queue takes to drain, or
 * ageing stops being a floor under starvation and becomes the order itself.
 * At 500 ms it was: a full zoom-out queues ~2,300 chunks against one scan
 * slot, about seven seconds of work, and a chunk at the fog edge (2.4x the
 * distance of the middle of the screen) overtook the middle after 640 ms.
 * The screen filled as a ring — the middle first, then the far edge, and the
 * band between them last (2026-09-12). At four seconds the same overtake
 * takes five, so a seven-second drain stays nearest-first almost throughout,
 * while a chunk passed over for sixteen seconds is still lifted well before
 * that.
 *
 * Everything waiting ages, holes included: a hole is not exempt from being
 * passed over, it is only easier to notice when it is.
 */
export const AGE_HALF_MS: f64 = 4000;

/**
 * The aged distance of every slot at `now`, computed once per sort rather
 * than inside the comparator: a comparison sort asks the key n·log n times,
 * and each ask was a power, so a full-recheck frame over two and a half
 * thousand chunks spent fifty thousand `2 ** x` on ordering alone. Sized
 * to the flag lane, which is the table's capacity.
 */
let keys = FIXED_ALLOC.take(Float64Array, 4096);

function fill_keys(lanes: PriorityLanes, now: f64): Float64Array {
  const n = lanes.flags.length;
  if (keys.length < n) {
    FIXED_ALLOC.give(keys);
    keys = FIXED_ALLOC.take(Float64Array, n);
  }
  const { lead_distance2, pending_since } = lanes;
  for (let i = 0; i < n; i++) {
    const since = pending_since[i];
    keys[i] = since < 0 ? lead_distance2[i] : lead_distance2[i] * 2 ** (-2 * (now - since) / AGE_HALF_MS);
  }
  return keys;
}

/**
 * The ordering of slots at `now`: slot holders, then what is on screen, then
 * nearest the place the view is heading, and only at equal distance does a
 * chunk with nothing drawn go before one that is merely coarse.
 *
 * Off screen, the order reverses: farthest first. Off-screen work is a
 * release, and a release exists to give memory back, so it should take it
 * from what is least likely to be wanted again — the chunk farthest from
 * where the view is heading. Releasing nearest-first did the opposite: after
 * a short stay zoomed in, a wide zoom-out found the near chunks already
 * gone and the far ones still holding their meshes, drawn as a ring of
 * stale terrain beyond an empty band (2026-09-12). Build nearest-first,
 * release farthest-first.
 *
 * Distance outranks emptiness on purpose. Ordering the two the other way
 * round — every hole anywhere on screen before any refinement — meant that
 * ground a few metres away stayed visibly blocky while chunks at the fog edge
 * filled in, because "already drawn" read as "done". What a viewer wants is
 * the nearest thing to improve next, whether improving means appearing or
 * sharpening.
 *
 * `now` is taken once per sort, and the keys with it — keys that moved
 * while a sort ran would make the comparator inconsistent. Ageing still
 * cannot lift anything out of the frustum class, so a release can never
 * pass a build.
 */
export function priority_order(lanes: PriorityLanes, now: f64): (a: u32, b: u32) => number {
  const flags = lanes.flags;
  const key = fill_keys(lanes, now);
  return (a, b) => {
    const fa = flags[a];
    const fb = flags[b];
    const holds = (fb & HOLDS_SLOT) - (fa & HOLDS_SLOT);
    if (holds !== 0) return holds;
    const on = (fb & IN_FRUSTUM) - (fa & IN_FRUSTUM);
    if (on !== 0) return on;
    const near = (fa & IN_FRUSTUM) !== 0 ? key[a] - key[b] : key[b] - key[a];
    if (near !== 0) return near;
    return (fb & EMPTY) - (fa & EMPTY);
  };
}
