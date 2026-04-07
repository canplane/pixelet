// @pxl/web/modules/camera/ray.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { add3, copy3, cross3, floor3n, mul3n, norm3, set3, sub3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import type { RenderCameraState, RenderWorldState } from "@/render/types.ts";
import { get_ele, with_ele } from "@/modules/world/terrain/use-elevation.ts";
import { get_local_world_origin } from "../world/frame/coord.ts";

const $hit: V3 = [0, 0, 0];
const $v: V3 = [0, 0, 0];
const $p: PVF3 = { i: 0n, f: 0n };
const $origin: V3 = [0, 0, 0];
const $dir: V3 = [0, 0, 0];

const $0: V3 = [0, 0, 0];
const $1: V3 = [0, 0, 0];
const $2: V3 = [0, 0, 0];

const STEP: f64 = 2 * U_PER_CELL;
const MAX_STEPS: u32 = 200;
const BIN_ITER: u32 = 6;
const HIT_EPS: f64 = 0.1 * U_PER_CELL;

const INTERACT_RADIUS_CELL: f64 = 32 / U_PER_CELL - .707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

function rotate_z_inv(v: Readonly<V3>, azimuth: f64, out: V3 = [0, 0, 0]): V3 {
  const c = Math.cos(azimuth);
  const s = Math.sin(azimuth);
  const x = v[0];
  const y = v[1];

  out[0] = c * x + s * y;
  out[1] = -s * x + c * y;
  out[2] = v[2];
  return out;
}

function make_local_world_ray(
  ndc: readonly [f64, f64],
  camera: RenderCameraState,
  world: RenderWorldState,
  out_origin: V3,
  out_dir: V3,
): void {
  const forward = norm3(sub3(camera.target, camera.eye, $0), $0);
  const right = norm3(cross3(forward, camera.up, $1), $1);
  const upn = norm3(cross3(right, forward, $2), $2);

  const tan_y = Math.tan(camera.fov_y_rad * 0.5);
  const tan_x = tan_y * camera.aspect;

  out_dir[0] = forward[0] + right[0] * ndc[0] * tan_x + upn[0] * ndc[1] * tan_y;
  out_dir[1] = forward[1] + right[1] * ndc[0] * tan_x + upn[1] * ndc[1] * tan_y;
  out_dir[2] = forward[2] + right[2] * ndc[0] * tan_x + upn[2] * ndc[1] * tan_y;
  norm3(out_dir, out_dir);

  copy3(camera.eye, out_origin);
  rotate_z_inv(out_origin, world.world_azimuth, out_origin);
  out_origin[0] -= world.local_world_offset[0];
  out_origin[1] -= world.local_world_offset[1];
  out_origin[2] -= world.local_world_offset[2];

  rotate_z_inv(out_dir, world.world_azimuth, out_dir);
  norm3(out_dir, out_dir);
}

export function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

function ray_terrain_delta_at_t(origin: Readonly<V3>, dir: Readonly<V3>, t: f64, map_origin: IVec): f64 | null {
  mul3n(dir, t, $0);
  add3(origin, $0, $hit);

  projection.project($p, $hit[0], $hit[1], $hit[2]);
  $p.i = ivec.add($p.i, map_origin);

  const ele: f64 | null = get_ele($p.i, $p.f);
  if (ele === null) return null;

  const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
  const visual_z: f64 = ele - origin_z;
  return $hit[2] - visual_z;
}

function ray_hit_terrain(origin: Readonly<V3>, dir: Readonly<V3>, map_origin: IVec): PVF3 | null {
  let t = 0;
  let hit_t = -1;

  for (let i = 0; i < MAX_STEPS; i++, t += STEP) {
    const delta: f64 | null = ray_terrain_delta_at_t(origin, dir, t, map_origin);
    if (delta === null) continue;
    if (delta < HIT_EPS) {
      hit_t = t;
      break;
    }
  }

  if (hit_t === -1) return null;

  let min = hit_t - STEP;
  let max = hit_t;

  for (let i = 0; i < BIN_ITER; i++) {
    const mid: f64 = (min + max) * 0.5;
    const delta: f64 | null = ray_terrain_delta_at_t(origin, dir, mid, map_origin);
    if (delta === null) {
      min = mid;
      continue;
    }
    delta < HIT_EPS ? (max = mid) : (min = mid);
  }

  mul3n(dir, max, $0);
  add3(origin, $0, $hit);

  set3($hit[0], $hit[1], 0, $v);
  floor3n($v, U_PER_CELL, $v);

  const p: PVF3 = projection.project({ i: 0n, f: 0n }, $v[0], $v[1], $v[2]);
  p.i = ivec.add(p.i, map_origin);

  return with_ele(p, p.i, p.f);
}

export function ray_pick_terrain(
  ndc: readonly [f64, f64],
  camera: RenderCameraState,
  world: RenderWorldState,
  map_center: PVF3 | null,
): PVF3 | null {
  if (!map_center) return null;

  const map_origin = get_local_world_origin(map_center);

  make_local_world_ray(ndc, camera, world, $origin, $dir);

  const hit = ray_hit_terrain($origin, $dir, map_origin);
  if (!hit) return null;
  if (!in_interact_radius(hit, map_center)) return null;

  return hit;
}
