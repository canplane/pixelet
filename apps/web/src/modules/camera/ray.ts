// @pxl/web/modules/camera/ray.ts

import * as THREE from "three";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { floor3n, set3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import { get_ele, with_ele } from "@/modules/world/terrain/use-elevation.ts";
import { get_local_world_origin } from "../world/frame/coord.ts";

const $hit = new THREE.Vector3();
const $v: V3 = [0, 0, 0];
const $p: PVF3 = { i: 0n, f: 0n };
const $origin = new THREE.Vector3();
const $dir = new THREE.Vector3();
const $m = new THREE.Matrix4();

const STEP: f64 = 2 * U_PER_CELL;
const MAX_STEPS: u32 = 200;
const BIN_ITER: u32 = 6;
const HIT_EPS: f64 = 0.1 * U_PER_CELL;

const INTERACT_RADIUS_CELL: f64 = 32 / U_PER_CELL - .707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

export function to_local_world_ray(out: THREE.Ray, ray: THREE.Ray, world_frame: THREE.Group | null, local_world_frame: THREE.Group | null): THREE.Ray {
  $m.identity();

  if (local_world_frame) {
    $m.copy(local_world_frame.matrixWorld);
  } else if (world_frame) {
    $m.copy(world_frame.matrixWorld);
  }

  $m.invert();

  $origin.copy(ray.origin).applyMatrix4($m);
  $dir.copy(ray.direction).transformDirection($m).normalize();
  return out.copy(ray).set($origin, $dir);
}

export function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

function ray_terrain_delta_at_t(ray: THREE.Ray, t: f64, map_origin: IVec): f64 | null {
  ray.at(t, $hit);

  projection.project($p, $hit.x, $hit.y, $hit.z);
  $p.i = ivec.add($p.i, map_origin);

  const ele: f64 | null = get_ele($p.i, $p.f);
  if (ele === null) return null;
  const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
  const visual_z: f64 = ele - origin_z;
  return $hit.z - visual_z;
}

function ray_hit_terrain(ray: THREE.Ray, map_origin: IVec): PVF3 | null {
  let t = 0;
  let hit_t = -1;

  for (let i = 0; i < MAX_STEPS; i++, t += STEP) {
    const delta: f64 | null = ray_terrain_delta_at_t(ray, t, map_origin);
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
    const delta: f64 | null = ray_terrain_delta_at_t(ray, mid, map_origin);
    if (delta === null) {
      min = mid;
      continue;
    }
    delta < HIT_EPS ? (max = mid) : (min = mid);
  }

  ray.at(max, $hit);
  set3($hit.x, $hit.y, 0, $v);
  floor3n($v, U_PER_CELL, $v);

  const p: PVF3 = projection.project({ i: 0n, f: 0n }, $v[0], $v[1], $v[2]);
  p.i = ivec.add(p.i, map_origin);

  return with_ele(p, p.i, p.f);
}

export function ray_pick_terrain(
  out: THREE.Ray,
  ray: THREE.Ray,
  world_frame: THREE.Group | null,
  local_world_frame: THREE.Group | null,
  map_center: PVF3 | null,
): PVF3 | null {
  if (!map_center) return null;
  const map_origin = get_local_world_origin(map_center);

  const local_ray = to_local_world_ray(out, ray, world_frame, local_world_frame);
  const hit = ray_hit_terrain(local_ray, map_origin);
  if (!hit) return null;
  if (!in_interact_radius(hit, map_center)) return null;

  return hit;
}
