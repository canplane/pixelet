// @pxl/web/modules/camera/terrain_gate.ts

/**
 * How far a camera control gets before terrain is in the way.
 *
 * A pure function of dist/polar/azimuth, applied to those values in
 * store.ts's set_view_state rather than to the position they are damped
 * into — dist is what the scale bar reads, so a control that failed to take
 * effect further down would move the bar while the view stood still.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { get_local_world_origin } from "@/modules/world/frame/coord.ts";
import { get_ele_at_local } from "@/modules/world/terrain/elevation.ts";
import { camera_cfg } from "./config.ts";
import { orbit_local_offset, rotate2 } from "./orbit.ts";

const { MIN_POLAR } = camera_cfg.view;

/** One cell, so a bicubic elevation sample disagreeing slightly with the
 * voxel it produced cannot put the eye inside a block. */
const EYE_CLEARANCE_U: f64 = 1;

/**
 * How far above the target the eye must sit to clear the ground beneath it,
 * or null when the tile under it has not loaded — an unarrived sample says
 * nothing.
 */
function required_rise(dist: f64, polar: f64, azimuth: f64): f64 | null {
  const map_center = useMapStore.getState().map_center;
  if (!map_center) return null;

  const map_origin = get_local_world_origin(map_center);
  const local: PVF3 = { i: ivec.sub(map_center.i, map_origin), f: map_center.f };
  const { horiz_dist } = orbit_local_offset(dist, polar);
  // The camera never rotates; azimuth spins the world beneath it, so
  // un-rotating the camera-local offset puts it in the ground's frame.
  const offset = rotate2(0, -horiz_dist, -azimuth);

  const ground = get_ele_at_local(
    TO_XY(L3_IDX(map_origin)),
    pvf3.to_real(pvf3.X(local)) + offset.x,
    pvf3.to_real(pvf3.Y(local)) + offset.y,
  );
  return ground === null ? null : ground + EYE_CLEARANCE_U - pvf3.to_real(pvf3.Z(local));
}

/** Whether these three controls would put the eye inside the terrain. */
export function destination_blocked(dist: f64, polar: f64, azimuth: f64): boolean {
  const rise = required_rise(dist, polar, azimuth);
  return rise !== null && orbit_local_offset(dist, polar).vert_dist < rise;
}

/** Testing only where a step lands lets the camera tunnel: use_camera.ts
 * damps toward the new state and visits everything between. The straight
 * line in control space is the path it takes. */
const STEP_SAMPLES = 16;

export type ViewTriple = { dist: f64; polar: f64; azimuth: f64 };

/**
 * Steepening the pitch moves the eye, which changes the ground under it, so
 * the answer is implicit. Two passes settle it: the eye travels less and
 * less as the pitch approaches vertical, and the remainder is smaller than
 * the cell the height was read from.
 */
const CLIMB_PASSES = 2;

/**
 * The shallowest pitch that clears terrain, between `polar` and straight
 * down, or straight down if nothing does.
 *
 * Solved rather than searched. The eye sits `dist * cos(polar)` above the
 * target, so clearing a required rise `h` needs `acos(h / dist)` — smooth in
 * `h`, which matters more than it sounds: a scan returns one of a fixed set
 * of angles, so its output steps even where the terrain does not, and the
 * camera judders on flat ground it is merely passing over.
 *
 * There is nearly always an answer. At MIN_POLAR the eye is directly above
 * the target and only terrain higher than `target + dist` could block it.
 */
export function climb_polar(dist: f64, polar: f64, azimuth: f64): f64 {
  let at = polar;
  for (let i = 0; i < CLIMB_PASSES; i++) {
    const rise = required_rise(dist, at, azimuth);
    if (rise === null || rise <= 0) break;
    // Only ever steepens: an eye already high enough is left where the
    // controls put it.
    const solved = Math.acos(Math.min(1, rise / dist));
    at = Math.max(MIN_POLAR, Math.min(at, solved));
  }
  return at;
}

export function allowed_step(
  dist: f64,
  polar: f64,
  azimuth: f64,
  from_dist: f64,
  from_polar: f64,
  from_azimuth: f64,
): ViewTriple {
  // Checked first so the setting skips the heightfield walk, not its result.
  // `climb` grants every control too — it answers terrain by pitching the
  // view, not by holding a control back.
  if (useSettingsStore.getState().camera_terrain !== "stop") return { dist, polar, azimuth };
  if (destination_blocked(from_dist, from_polar, from_azimuth)) return { dist, polar, azimuth };

  // The short way round, matching how use_frame_rotation.ts damps azimuth.
  const turn = wrap(azimuth - from_azimuth, -Math.PI, Math.PI);
  let last: ViewTriple = { dist: from_dist, polar: from_polar, azimuth: from_azimuth };
  for (let i = 1; i <= STEP_SAMPLES; i++) {
    const t = i / STEP_SAMPLES;
    const at: ViewTriple = {
      dist: from_dist + (dist - from_dist) * t,
      polar: from_polar + (polar - from_polar) * t,
      azimuth: from_azimuth + turn * t,
    };
    // The last clear sample, not the crossing point: landing on the boundary
    // leaves the next frame's rounding to decide which side the eye is on.
    if (destination_blocked(at.dist, at.polar, at.azimuth)) return last;
    last = at;
  }
  return { dist, polar, azimuth };
}
