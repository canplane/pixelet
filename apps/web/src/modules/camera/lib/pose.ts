// @pxl/web/modules/camera/lib/pose.ts

import * as THREE from "three";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { fill3, type V3 } from "@pxl/spec/space/v3.ts";

/** Native Z-up direction for the web world renderer. */
export const CAMERA_UP: V3 = [0, 0, 1];

/**
 * Camera view intent.
 *
 * This is not the raw physical camera transform.
 * It stores the user/system-facing view parameters used to derive a pose.
 */
export type ViewState = {
  /** Camera-to-target distance. */
  dist: f64;
  /** Compass bearing in degrees, normalized to [-180, 180], north = +Y. */
  bearing: f64;
  /** Orbit polar angle in radians. */
  polar_angle: f64;
  /** Vertical camera offset from the ground-aligned target. */
  height: f64;
};

/** Derived camera pose in native Z-up world coordinates. */
export type CameraPose = {
  /** Ground-aligned local look-at point. */
  look_at: V3;
  /** Camera position paired with the derived look-at point. */
  pos: V3;
};

/** Mutable fly-to destination used by camera interpolation. */
export type CameraFlyTarget = {
  /** Final camera position. */
  dst_pos: THREE.Vector3;
  /** Final look-at target. */
  dst_look_at: THREE.Vector3;
};

/**
 * Derives a local Z-up camera pose from canonical camera view inputs.
 *
 * The returned vectors are already in the native web scene basis and do not
 * require any Y-up adaptation.
 */
export function derive_camera_pose(
  map_center: PVF3,
  map_origin: IVec,
  dist: f64,
  bearing: f64,
  polar_angle: f64,
): CameraPose {
  const i: IVec = ivec.sub(map_center.i, map_origin);
  const look_at: V3 = projection.unproject(fill3(0), i, map_center.f);

  const bearing_rad: f64 = bearing * (Math.PI / 180);
  const horiz_dist: f64 = dist * Math.sin(polar_angle);
  const vert_dist: f64 = dist * Math.cos(polar_angle);

  return {
    look_at,
    pos: [
      look_at[0] - horiz_dist * Math.sin(bearing_rad),
      look_at[1] - horiz_dist * Math.cos(bearing_rad),
      look_at[2] + vert_dist,
    ],
  };
}
