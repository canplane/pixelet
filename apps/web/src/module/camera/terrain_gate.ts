// @pxl/web/module/camera/terrain_gate.ts

/**
 * How far a camera control gets before terrain is in the way.
 *
 * A pure function of the pose, applied to the values the controls write
 * rather than to the position they are damped into — dist is what the scale
 * bar reads, so a control that failed to take effect further down would move
 * the bar while the view stood still.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { useMapStore } from "@/module/map/store.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { get_local_world_origin, local_world_to_canonical } from "@/module/world/frame/coord.ts";
import { get_ele_at_local } from "@/module/world/terrain/elevation.ts";
import { camera_cfg } from "./config.ts";
import { orbit_local_offset, rotate2 } from "./orbit.ts";

const { MIN_POLAR } = camera_cfg.view;

/** One cell, so a bicubic elevation sample disagreeing slightly with the
 * voxel it produced cannot put the eye inside a block. */
const EYE_CLEARANCE_U: f64 = 1;

export type ViewTriple = { dist: f64; polar: f64; azimuth: f64 };

/**
 * Everything that decides where the eye is: the point it looks at, and the
 * orbit around it.
 *
 * The target belongs here because the eye is `target + orbit offset` and a
 * gate that reads only the second term can be walked through by moving the
 * first — which is what panning does. It is carried in the local render
 * frame (real u against `origin_page`), the frame the orbit offset and the
 * heightfield lookup already share, and the one a step can be interpolated
 * in.
 */
export type ViewPose = {
  /** `TO_XY(L3_IDX(map_origin))` for the frame `target` is expressed in. */
  origin_page: IVec;
  target: Readonly<V3>;
  dist: f64;
  polar: f64;
  azimuth: f64;
};

/**
 * The pose these controls describe while looking at `center`, or null before
 * the first fix, when there is no frame to express it in.
 *
 * Both ends of a step have to be built against one origin or the walk
 * between them crosses frames; the store's current centre supplies it for
 * every caller, and a candidate target is expressed against that same one.
 */
export function pose(view: ViewTriple, center?: PVF3 | null): ViewPose | null {
  const map_center = useMapStore.getState().map_center;
  const at = center ?? map_center;
  if (!map_center || !at) return null;

  const origin = get_local_world_origin(map_center);
  const local: PVF3 = { i: ivec.sub(at.i, origin), f: at.f };
  return {
    origin_page: TO_XY(L3_IDX(origin)),
    target: [pvf3.to_real(pvf3.X(local)), pvf3.to_real(pvf3.Y(local)), pvf3.to_real(pvf3.Z(local))],
    dist: view.dist,
    polar: view.polar,
    azimuth: view.azimuth,
  };
}

/**
 * How far above the target the eye must sit to clear the ground beneath it,
 * or null when the tile under it has not loaded — an unarrived sample says
 * nothing.
 */
function required_rise(at: ViewPose): f64 | null {
  const { horiz_dist } = orbit_local_offset(at.dist, at.polar);
  // The camera never rotates; azimuth spins the world beneath it, so
  // un-rotating the camera-local offset puts it in the ground's frame.
  const offset = rotate2(0, -horiz_dist, -at.azimuth);

  const ground = get_ele_at_local(at.origin_page, at.target[0] + offset.x, at.target[1] + offset.y);
  return ground === null ? null : ground + EYE_CLEARANCE_U - at.target[2];
}

/** How far below its clearance the eye sits: positive inside the terrain,
 * null where the ground is not known yet. */
function penetration(at: ViewPose): f64 | null {
  const rise = required_rise(at);
  return rise === null ? null : rise - orbit_local_offset(at.dist, at.polar).vert_dist;
}

/** Testing only where a step lands lets the camera tunnel: use_camera.ts
 * damps toward the new state and visits everything between. The straight
 * line in pose space is the path it takes. */
const STEP_SAMPLES = 16;

/**
 * As far along `from` -> `to` as terrain allows, in `stop` mode.
 *
 * The rule is that a step may not end up deeper in the ground than it
 * started. Outside terrain that is the plain "refuse to enter"; inside it,
 * every escape is still granted and the bar tightens on its own as one
 * succeeds. A blanket exemption for an already-blocked start — which is what
 * this used to do — opened the gate completely and left it open, since
 * nothing that followed could ever close it again.
 */
export function allowed_step(to: ViewPose, from: ViewPose): ViewPose {
  // Checked first so the setting skips the heightfield walk, not its result.
  // `climb` grants every control too — it answers terrain by pitching the
  // view, not by holding a control back.
  if (useSettingsStore.getState().camera_terrain !== "stop") return to;

  const start = penetration(from);
  const limit = start === null ? 0 : Math.max(0, start);

  // The short way round, matching how use_frame_rotation.ts damps azimuth.
  const turn = wrap(to.azimuth - from.azimuth, -Math.PI, Math.PI);
  let last: ViewPose = from;
  for (let i = 1; i <= STEP_SAMPLES; i++) {
    const t = i / STEP_SAMPLES;
    const at: ViewPose = {
      origin_page: from.origin_page,
      target: [
        from.target[0] + (to.target[0] - from.target[0]) * t,
        from.target[1] + (to.target[1] - from.target[1]) * t,
        from.target[2] + (to.target[2] - from.target[2]) * t,
      ],
      dist: from.dist + (to.dist - from.dist) * t,
      polar: from.polar + (to.polar - from.polar) * t,
      azimuth: from.azimuth + turn * t,
    };
    // The last clear sample, not the crossing point: landing on the boundary
    // leaves the next frame's rounding to decide which side the eye is on.
    const deep = penetration(at);
    if (deep !== null && deep > limit) return last;
    last = at;
  }
  // `to` itself rather than the t=1 sample, so an unobstructed step is the
  // exact values the control asked for.
  return to;
}

/** The orbit half, where the target holds still. */
export function allowed_view(to: ViewTriple, from: ViewTriple): ViewTriple {
  const from_pose = pose(from);
  const to_pose = pose(to);
  if (!from_pose || !to_pose) return to;
  const reached = allowed_step(to_pose, from_pose);
  return { dist: reached.dist, polar: reached.polar, azimuth: reached.azimuth };
}

/**
 * The pan half: the target moves and takes the eye with it.
 *
 * Panning is the one continuous control that is not in the view triple, so
 * it went through no gate at all and walking into a hill sideways simply
 * worked. Jumps do not come through here — a search result or a GPS recentre
 * is a destination the user named, not a path they dragged.
 */
export function allowed_pan(view: ViewTriple, to_center: PVF3): PVF3 {
  const from_pose = pose(view);
  const to_pose = pose(view, to_center);
  if (!from_pose || !to_pose) return to_center;

  const reached = allowed_step(to_pose, from_pose);
  if (reached === to_pose) return to_center;

  const map_origin = get_local_world_origin(useMapStore.getState().map_center);
  return local_world_to_canonical({ i: 0n, f: 0n }, reached.target, map_origin);
}

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
  const at = pose({ dist, polar, azimuth });
  if (!at) return polar;

  for (let i = 0; i < CLIMB_PASSES; i++) {
    const rise = required_rise(at);
    if (rise === null || rise <= 0) break;
    // Only ever steepens: an eye already high enough is left where the
    // controls put it.
    const solved = Math.acos(Math.min(1, rise / dist));
    at.polar = Math.max(MIN_POLAR, Math.min(at.polar, solved));
  }
  return at.polar;
}
