// @pxl/web/modules/camera/lib/camera-change.ts

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

export type CameraChangeSample = {
  target_x: f64;
  target_y: f64;
  target_z: f64;

  dist: f64;
  bearing: f64;
  polar_angle: f64;
};

export type CameraViewSample = CameraChangeSample & {
  height: f64;
};

export type CameraChangeDelta = {
  target_dx: f64;
  target_dy: f64;
  target_dz: f64;
  target_delta2: f64;

  dist_delta: f64;
  bearing_delta: f64;
  polar_delta: f64;
};

export type CameraChangeThreshold = {
  target_eps2: f64;
  dist_eps: f64;
  bearing_eps: f64;
  polar_eps: f64;
};

export const DEFAULT_CAMERA_CHANGE_THRESHOLD: CameraChangeThreshold = {
  target_eps2: 0.001 * 0.001,
  dist_eps: 0.001,
  bearing_eps: 0.1,
  polar_eps: 0.001,
};

export const USER_CAMERA_CHANGE_THRESHOLD: CameraChangeThreshold = {
  target_eps2: 0.001 * 0.001,
  dist_eps: Number.POSITIVE_INFINITY,
  bearing_eps: Number.POSITIVE_INFINITY,
  polar_eps: Number.POSITIVE_INFINITY,
};

export function calc_camera_change_delta(
  cur: Readonly<CameraChangeSample>,
  base: Readonly<CameraChangeSample>,
): CameraChangeDelta {
  const target_dx: f64 = cur.target_x - base.target_x;
  const target_dy: f64 = cur.target_y - base.target_y;
  const target_dz: f64 = cur.target_z - base.target_z;

  return {
    target_dx,
    target_dy,
    target_dz,
    target_delta2: target_dx * target_dx + target_dy * target_dy + target_dz * target_dz,

    dist_delta: Math.abs(cur.dist - base.dist),
    bearing_delta: Math.abs(cur.bearing - base.bearing),
    polar_delta: Math.abs(cur.polar_angle - base.polar_angle),
  };
}

export function is_camera_change_delta_over_threshold(
  delta: Readonly<CameraChangeDelta>,
  thres: Readonly<CameraChangeThreshold> = DEFAULT_CAMERA_CHANGE_THRESHOLD,
): boolean {
  return delta.target_delta2 > thres.target_eps2 ||
    delta.dist_delta > thres.dist_eps ||
    delta.bearing_delta > thres.bearing_eps ||
    delta.polar_delta > thres.polar_eps;
}

export function is_camera_changed(
  cur: Readonly<CameraChangeSample>,
  base: Readonly<CameraChangeSample>,
  thres: Readonly<CameraChangeThreshold> = DEFAULT_CAMERA_CHANGE_THRESHOLD,
): boolean {
  return is_camera_change_delta_over_threshold(calc_camera_change_delta(cur, base), thres);
}

export function calc_camera_view_sample(
  cam_x: f64,
  cam_y: f64,
  cam_z: f64,
  target_x: f64,
  target_y: f64,
  target_z: f64,
  polar_angle: f64,
  azimuthal_angle_rad: f64,
): CameraViewSample {
  const dx: f64 = cam_x - target_x;
  const dy: f64 = cam_y - target_y;
  const dz: f64 = cam_z - target_z;

  const dist: f64 = Math.sqrt(dx * dx + dy * dy + dz * dz);
  const height: f64 = cam_z - target_z;

  let bearing: f64 = -azimuthal_angle_rad * (180 / Math.PI);
  if (bearing <= -180) bearing += 360;
  if (bearing > 180) bearing -= 360;

  return {
    target_x,
    target_y,
    target_z,
    dist,
    height,
    bearing,
    polar_angle,
  };
}
