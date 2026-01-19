// @pxl/spec/space/adapter/three.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

/**
 * Coordinate System Conversion
 *
 * Source frame:
 *   Map-local ENU (meters)
 *   - X = East
 *   - Y = North
 *   - Z = Up (elevation)
 *   - X/Y axes are aligned with EPSG:3857 (Web Mercator meters)
 *
 * Target frame (Three.js):
 *   - X = East
 *   - Y = Up
 *   - Z = South
 *
 * Basis transform:
 *   (E, N, U)  →  (E, U, -N)
 */

export function to3(dst: V3, src: Readonly<V3> = dst): V3 {
  // 1-arg: in-place transform
  // 2-arg: src -> dst
  const enu_x: f64 = src[0];
  const enu_y: f64 = src[1];
  const enu_z: f64 = src[2];
  dst[0] = enu_x;
  dst[1] = enu_z;
  dst[2] = -enu_y;
  return dst;
}

export function from3(dst: V3, src: Readonly<V3> = dst): V3 {
  const yup_x: f64 = src[0];
  const yup_y: f64 = src[1];
  const yup_z: f64 = src[2];
  dst[0] = yup_x;
  dst[1] = -yup_z;
  dst[2] = yup_y;
  return dst;
}

export type CameraPose = {
  look_at: V3;
  pos: V3;
};

/** Derives a Three-local camera pose from canonical camera model inputs. */
export function derive_camera_pose(map_center: PVF3, map_origin: IVec, dist: f64, bearing: f64, polar_angle: f64): CameraPose {
  const i: IVec = ivec.sub(map_center.i, map_origin);
  const enu: V3 = projection.unproject([0, 0, 0], i, map_center.f);

  const look_at: V3 = to3(enu);
  const azimuth: f64 = (-bearing * Math.PI) / 180;

  return {
    look_at,
    pos: [
      look_at[0] + dist * Math.sin(polar_angle) * Math.sin(azimuth),
      look_at[1] + dist * Math.cos(polar_angle),
      look_at[2] + dist * Math.sin(polar_angle) * Math.cos(azimuth),
    ],
  };
}
