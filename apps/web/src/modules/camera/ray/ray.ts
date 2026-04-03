// @pxl/web/modules/camera/ray/ray.ts

import * as THREE from "three";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type FVec, type IVec, ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { floor3, type V3 } from "@pxl/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

const STEP: f64 = 2 * U_PER_CELL;
const MAX_STEPS: u32 = 200;
const BIN_ITER: u32 = 6;
const HIT_EPS: f64 = 0.1 * U_PER_CELL;

export type SampleEleFn = (i: IVec, f: FVec) => f64 | null;
export type GroundPointFn = (out: PVF3, i: IVec, f: FVec) => PVF3 | null;

export function ray_terrain_delta_at_t(
  ray: THREE.Ray,
  t: f64,
  map_origin: IVec,
  sample_ele: SampleEleFn,
): f64 {
  const hit: THREE.Vector3 = ray.at(t, new THREE.Vector3());

  const p: PVF3 = projection.project({ i: 0n, f: 0n }, hit.x, hit.y, hit.z);
  p.i = ivec.add(p.i, map_origin);

  const ele: f64 = sample_ele(p.i, p.f) ?? 0;
  const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
  const visual_z: f64 = ele - origin_z;
  return hit.z - visual_z;
}

export function ray_hit_terrain(
  ray: THREE.Ray,
  map_origin: IVec,
  sample_ele: SampleEleFn,
  ground_point: GroundPointFn,
): PVF3 | null {
  let t = 0;
  let hit_t = -1;

  for (let i = 0; i < MAX_STEPS; i++, t += STEP) {
    if (ray_terrain_delta_at_t(ray, t, map_origin, sample_ele) < HIT_EPS) {
      hit_t = t;
      break;
    }
  }

  if (hit_t === -1) return null;

  let min = hit_t - STEP;
  let max = hit_t;
  for (let i = 0; i < BIN_ITER; i++) {
    const mid: f64 = (min + max) * 0.5;
    ray_terrain_delta_at_t(ray, mid, map_origin, sample_ele) < HIT_EPS ? (max = mid) : (min = mid);
  }

  const hit_three: THREE.Vector3 = ray.at(max, new THREE.Vector3());
  const hit: V3 = [hit_three.x, hit_three.y, 0];
  const snapped: V3 = floor3(hit, U_PER_CELL);

  const p: PVF3 = projection.project({ i: 0n, f: 0n }, ...snapped);
  p.i = ivec.add(p.i, map_origin);

  return ground_point(p, p.i, p.f);
}
