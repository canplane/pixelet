// @pxl/web/render/layer/pipeline/chunk_raycast.ts

/** Pure raycast against one already-composited 64-wide chunk. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { CORE_PXL } from "./core_pool.ts";
import { type ChunkVolume } from "./volume.ts";
import { MaterialClass, SUB_CELL_NONE, unpack_sub_cell_fraction } from "./voxel.ts";
import { merge_column_runs } from "./voxel_scan.ts";

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

/**
 * One column's runs as lanes, read once: the module's walk of its own
 * volume (crates/pxl-core/src/scan.rs's ColumnRuns) — the same walk the
 * scan makes, so the raycast tests exactly the boxes the mesher emits —
 * or, where there is no core memory, the TypeScript walk copied into
 * the same lanes. The inset is packed as the band stores it: ox, oy, oz,
 * then sz, SUB_CELL_NONE meaning a full cell.
 */
export type ColumnRuns = {
  len: u32;
  z0: Int32Array;
  z1: Int32Array;
  material_class: Uint8Array;
  sub: Uint32Array;
};

const $ts_runs: ColumnRuns = { len: 0, z0: new Int32Array(64), z1: new Int32Array(64), material_class: new Uint8Array(64), sub: new Uint32Array(64) };
const $runs: ColumnRuns = { len: 0, z0: new Int32Array(0), z1: new Int32Array(0), material_class: new Uint8Array(0), sub: new Uint32Array(0) };

export function column_runs(volume: ChunkVolume, cx: u32, cy: u32): ColumnRuns {
  const p = CORE_PXL;
  if (p !== null) {
    const n = p.pxl_column_runs(volume.id, cx, cy);
    const buf = p.memory.buffer;
    const row = new Uint32Array(buf, p.pxl_column_runs_at(), 4);
    $runs.len = n;
    $runs.z0 = new Int32Array(buf, row[0], n);
    $runs.z1 = new Int32Array(buf, row[1], n);
    $runs.material_class = new Uint8Array(buf, row[2], n);
    $runs.sub = new Uint32Array(buf, row[3], n);
    return $runs;
  }
  const runs = merge_column_runs(volume, cx, cy);
  const out = $ts_runs;
  if (runs.length > out.z0.length) {
    const cap = 1 << Math.ceil(Math.log2(runs.length));
    out.z0 = new Int32Array(cap);
    out.z1 = new Int32Array(cap);
    out.material_class = new Uint8Array(cap);
    out.sub = new Uint32Array(cap);
  }
  out.len = runs.length;
  for (let i = 0; i < runs.length; i++) {
    const r = runs[i];
    out.z0[i] = r.z0;
    out.z1[i] = r.z1;
    out.material_class[i] = r.material_class;
    out.sub[i] = (r.sub_ox | (r.sub_oy << 8) | (r.sub_oz << 16) | (r.sub_sz << 24)) >>> 0;
  }
  return out;
}

/** The box run `i` of `runs` is, in chunk-local units, into `min`/`max` —
 * voxel_scan.ts's run_to_box over lanes. */
export function run_box(cx: u32, cy: u32, runs: ColumnRuns, i: u32, min: V3, max: V3): void {
  const sub = runs.sub[i];
  const sub_sz = sub >>> 24;
  if (sub_sz === SUB_CELL_NONE) {
    min[0] = cx * U_PER_CELL;
    min[1] = cy * U_PER_CELL;
    min[2] = runs.z0[i] * U_PER_CELL;
    max[0] = (cx + 1) * U_PER_CELL;
    max[1] = (cy + 1) * U_PER_CELL;
    max[2] = (runs.z1[i] + 1) * U_PER_CELL;
    return;
  }
  const sz = unpack_sub_cell_fraction(sub_sz) * U_PER_CELL;
  min[0] = (cx + unpack_sub_cell_fraction(sub & 255)) * U_PER_CELL;
  min[1] = (cy + unpack_sub_cell_fraction((sub >>> 8) & 255)) * U_PER_CELL;
  min[2] = (runs.z0[i] + unpack_sub_cell_fraction((sub >>> 16) & 255)) * U_PER_CELL;
  max[0] = min[0] + sz;
  max[1] = min[1] + sz;
  max[2] = min[2] + sz;
}

const CHUNK_W: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/**
 * Cell-DDA through one chunk, testing exactly the boxes the mesher emits:
 * the scan's own column walk (column_runs), the box a run is (run_box).
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
    const runs = column_runs(volume, cx, cy);
    for (let i: u32 = 0; i < runs.len; i++) {
      run_box(cx, cy, runs, i, $box_min, $box_max);
      $box_min[0] += chunk_ox;
      $box_min[1] += chunk_oy;
      $box_max[0] += chunk_ox;
      $box_max[1] += chunk_oy;

      const hit = ray_aabb_hit(ray, $box_min, $box_max);
      if (!hit || hit.t < 0 || (best && hit.t >= best.hit.t)) continue;
      best = { hit, material_class: runs.material_class[i] as MaterialClass };
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
