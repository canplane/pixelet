// @pxl/web/render/layer/pipeline/chunk_raycast.ts

/** Pure raycast against one already-composited 64-wide chunk. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { type ChunkVolume } from "./volume.ts";
import { MaterialClass } from "./voxel.ts";
import { merge_column_runs, run_to_box } from "./voxel_scan.ts";

const $aabb_o: V3 = [0, 0, 0];
const $aabb_d: V3 = [0, 0, 0];
const $box_min: V3 = [0, 0, 0];
const $box_max: V3 = [0, 0, 0];

export type WorldHit = {
  material_class: MaterialClass;
  face_normal: V3;
  hit_point: V3;
  t: f64;
  box_min: V3;
  box_max: V3;
};

/** Minimal structural ray contract; THREE.Ray satisfies it without an adapter. */
export type RayLike = {
  readonly origin: { readonly x: f64; readonly y: f64; readonly z: f64 };
  readonly direction: { readonly x: f64; readonly y: f64; readonly z: f64 };
};

/** Ray/AABB slab test. */
export function ray_aabb_hit(ray: RayLike, min: Readonly<V3>, max: Readonly<V3>): { t: f64; axis: 0 | 1 | 2; normal_sign: f64 } | null {
  const o = $aabb_o;
  const d = $aabb_d;
  o[0] = ray.origin.x;
  o[1] = ray.origin.y;
  o[2] = ray.origin.z;
  d[0] = ray.direction.x;
  d[1] = ray.direction.y;
  d[2] = ray.direction.z;

  let t_enter = -Infinity;
  let t_exit = Infinity;
  let enter_axis: 0 | 1 | 2 = 0;
  let enter_sign: f64 = -1;

  for (let axis = 0; axis < 3; axis++) {
    const dir = d[axis as 0 | 1 | 2];
    if (Math.abs(dir) < 1e-12) {
      if (o[axis] < min[axis] || o[axis] > max[axis]) return null;
      continue;
    }

    const t_lo: f64 = (min[axis] - o[axis]) / dir;
    const t_hi: f64 = (max[axis] - o[axis]) / dir;
    const near: f64 = dir > 0 ? t_lo : t_hi;
    const far: f64 = dir > 0 ? t_hi : t_lo;
    if (near > t_enter) {
      t_enter = near;
      enter_axis = axis as 0 | 1 | 2;
      enter_sign = dir > 0 ? -1 : 1;
    }
    t_exit = Math.min(t_exit, far);
    if (t_enter > t_exit) return null;
  }

  if (t_exit < 0) return null;
  return { t: t_enter, axis: enter_axis, normal_sign: enter_sign };
}

const CHUNK_W: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/**
 * Cell-DDA through one chunk, testing exactly the boxes emitted by
 * chunk_mesher.ts through the shared merge_column_runs/run_to_box path.
 */
export function ray_hit_chunk(ray: RayLike, volume: ChunkVolume, chunk_ox: f64, chunk_oy: f64, max_dist: f64): WorldHit | null {
  const ox: f64 = ray.origin.x - chunk_ox;
  const oy: f64 = ray.origin.y - chunk_oy;
  const dx: f64 = ray.direction.x;
  const dy: f64 = ray.direction.y;
  const win_min: V3 = [0, 0, 0];
  const win_max: V3 = [0, 0, 0];

  function test_cell(cx: u32, cy: u32): WorldHit | null {
    let best: { hit: NonNullable<ReturnType<typeof ray_aabb_hit>>; material_class: MaterialClass } | null = null;

    // Same cross-band runs as the mesher: a cache boundary cannot manufacture
    // a hidden face or a different ray target.
    for (const run of merge_column_runs(volume, cx, cy)) {
      const box = run_to_box(cx, cy, run);
      $box_min[0] = box.min[0] + chunk_ox;
      $box_min[1] = box.min[1] + chunk_oy;
      $box_min[2] = box.min[2];
      $box_max[0] = box.max[0] + chunk_ox;
      $box_max[1] = box.max[1] + chunk_oy;
      $box_max[2] = box.max[2];

      const hit = ray_aabb_hit(ray, $box_min, $box_max);
      if (!hit || hit.t < 0 || (best && hit.t >= best.hit.t)) continue;
      best = { hit, material_class: run.material_class };
      win_min[0] = $box_min[0];
      win_min[1] = $box_min[1];
      win_min[2] = $box_min[2];
      win_max[0] = $box_max[0];
      win_max[1] = $box_max[1];
      win_max[2] = $box_max[2];
    }

    if (!best || best.hit.t > max_dist) return null;
    const face_normal: V3 = [0, 0, 0];
    face_normal[best.hit.axis] = best.hit.normal_sign;
    const t = best.hit.t;
    return {
      material_class: best.material_class,
      face_normal,
      hit_point: [
        ray.origin.x + ray.direction.x * t,
        ray.origin.y + ray.direction.y * t,
        ray.origin.z + ray.direction.z * t,
      ],
      t,
      box_min: [...win_min],
      box_max: [...win_max],
    };
  }

  if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) {
    const cx: f64 = Math.floor(ox / U_PER_CELL);
    const cy: f64 = Math.floor(oy / U_PER_CELL);
    if (cx < 0 || cx >= CHUNK_CELL_SPAN || cy < 0 || cy >= CHUNK_CELL_SPAN) return null;
    return test_cell(cx as u32, cy as u32);
  }

  let cell_x: f64 = Math.floor(ox / U_PER_CELL);
  let cell_y: f64 = Math.floor(oy / U_PER_CELL);
  const step_x: f64 = dx > 0 ? 1 : -1;
  const step_y: f64 = dy > 0 ? 1 : -1;
  const t_delta_x: f64 = Math.abs(dx) < 1e-9 ? Infinity : U_PER_CELL / Math.abs(dx);
  const t_delta_y: f64 = Math.abs(dy) < 1e-9 ? Infinity : U_PER_CELL / Math.abs(dy);
  let t_max_x: f64 = Math.abs(dx) < 1e-9 ? Infinity : ((dx > 0 ? cell_x + 1 : cell_x) * U_PER_CELL - ox) / dx;
  let t_max_y: f64 = Math.abs(dy) < 1e-9 ? Infinity : ((dy > 0 ? cell_y + 1 : cell_y) * U_PER_CELL - oy) / dy;
  const max_cells: u32 = Math.ceil(Math.min(max_dist, CHUNK_W * 1.5) / U_PER_CELL) + 2;

  for (let i = 0; i < max_cells; i++) {
    if (cell_x >= 0 && cell_x < CHUNK_CELL_SPAN && cell_y >= 0 && cell_y < CHUNK_CELL_SPAN) {
      const found = test_cell(cell_x as u32, cell_y as u32);
      if (found) return found;
    }
    if (t_max_x < t_max_y) {
      cell_x += step_x;
      t_max_x += t_delta_x;
    } else {
      cell_y += step_y;
      t_max_y += t_delta_y;
    }
  }
  return null;
}
