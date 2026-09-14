// @pxl/web/module/world/view.ts

/**
 * The view as drawn: the camera and the frame the world stands in, as
 * doubles the renderer, the module's frame, the placement passes and the
 * pointer's ray all read.
 *
 * The model is the orbit camera's (camera/orbit.ts): the eye sits at
 * `dist` from the origin, `polar` down from vertical, on the −Y side,
 * looking at the origin with Z up — and never turns. The world turns
 * under it instead: the root frame everything is placed in is the map's
 * azimuth about Z, then a translation that puts the map centre at the
 * origin. Each of those is damped toward its control by its own step
 * (camera/use_camera.ts, frame/use_view_frame.ts), and whichever moved
 * composes the matrices again.
 *
 * Three's scene graph held this as groups — view frame, world frame,
 * local world frame, the anchor root — whose composed `matrixWorld` was
 * only current after three's own render, a frame after the tick that read
 * it. Held here the matrices are current the moment a step writes them.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import {
  type Mat4,
  mat4,
  mat4_frustum_planes,
  mat4_invert,
  mat4_look_at,
  mat4_mul,
  mat4_perspective,
  mat4_rotation_z,
  mat4_transform_direction,
  mat4_transform_point,
  mat4_translation,
  type Vec3,
} from "@xpute/core/kit/mat4.ts";

import { camera_cfg } from "@/module/camera/config.ts";
import { render_cfg } from "@/module/world/environment/config.ts";
import { orbit_local_offset } from "@/module/camera/orbit.ts";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

const $rot = mat4();
const $trans = mat4();
const $origin: Vec3 = { x: 0, y: 0, z: 0 };
const UP: Vec3 = { x: 0, y: 0, z: 1 };

export const view = {
  // ---- the controls as drawn, written by their damping steps ----
  /** The eye's distance from the origin and its angle down from vertical,
   * the climb included. */
  dist: camera_cfg.view.MAX_DIST,
  polar: 0,
  /** The map's turn about Z. */
  azimuth: 0,
  /** Where the root frame's origin stands: minus the map centre's offset
   * from the map origin, in units. */
  tx: 0,
  ty: 0,
  tz: 0,

  // ---- the viewport ----
  width: 1,
  height: 1,
  fov: FOV,
  near: NEAR,
  far: FAR_CLIP,

  // ---- composed ----
  /** The eye, in the view's own frame. */
  eye: { x: 0, y: 0, z: 1 } as Vec3,
  /** The eye in the root frame, where everything is placed. */
  eye_local: { x: 0, y: 0, z: 1 } as Vec3,
  /** View-frame to camera, and back: the camera's own matrix. */
  view_matrix: mat4(),
  inv_view_matrix: mat4(),
  /** Camera to GL clip. */
  proj: mat4(),
  view_proj: mat4(),
  inv_view_proj: mat4(),
  /** Root frame to view frame, and back. */
  root: mat4(),
  inv_root: mat4(),
  /** Root frame to GL clip. */
  local_view_proj: mat4(),
  /** The frustum's planes in the view frame, normals in, as the module's
   * frame takes them. */
  planes: new Float64Array(24),
};

const IDENTITY: Mat4 = mat4();

/** The viewport in CSS pixels; the projection follows it. */
export function view_resize(width: f64, height: f64): void {
  if (width === view.width && height === view.height) return;
  view.width = Math.max(1, width);
  view.height = Math.max(1, height);
  view_compose();
}

/** Composes the matrices from the controls as they stand. */
export function view_compose(): void {
  const v = view;
  const { horiz_dist, vert_dist } = orbit_local_offset(v.dist, v.polar);
  v.eye.x = 0;
  v.eye.y = -horiz_dist;
  v.eye.z = vert_dist;
  mat4_look_at(v.view_matrix, v.eye, $origin, UP);
  mat4_invert(v.inv_view_matrix, v.view_matrix);
  mat4_perspective(v.proj, (v.fov * Math.PI) / 180, v.width / v.height, v.near, v.far);
  mat4_mul(v.view_proj, v.proj, v.view_matrix);
  mat4_invert(v.inv_view_proj, v.view_proj);
  mat4_mul(v.root, mat4_rotation_z($rot, v.azimuth), mat4_translation($trans, v.tx, v.ty, v.tz));
  mat4_invert(v.inv_root, v.root);
  mat4_mul(v.local_view_proj, v.view_proj, v.root);
  mat4_frustum_planes(v.planes, v.view_proj);
  mat4_transform_point(v.eye_local, v.inv_root, v.eye.x, v.eye.y, v.eye.z);
}

const $far: Vec3 = { x: 0, y: 0, z: 0 };

/**
 * The ray under a screen point, in the root frame: `ndc_x`/`ndc_y` in
 * [−1, 1], x right and y up. `origin` is the eye and `direction` unit.
 */
export function view_ray_local(ndc_x: f64, ndc_y: f64, origin: Vec3, direction: Vec3): void {
  const v = view;
  origin.x = v.eye_local.x;
  origin.y = v.eye_local.y;
  origin.z = v.eye_local.z;
  // A point on the far plane, back through the projection and the root.
  mat4_transform_point($far, v.inv_view_proj, ndc_x, ndc_y, 1);
  mat4_transform_point($far, v.inv_root, $far.x, $far.y, $far.z);
  mat4_transform_direction(direction, IDENTITY, $far.x - origin.x, $far.y - origin.y, $far.z - origin.z);
}

/** Whether a sphere in the view frame meets the frustum. */
export function view_sees_sphere(x: f64, y: f64, z: f64, radius: f64): boolean {
  const p = view.planes;
  for (let k = 0; k < 24; k += 4) {
    if (p[k] * x + p[k + 1] * y + p[k + 2] * z + p[k + 3] < -radius) return false;
  }
  return true;
}
