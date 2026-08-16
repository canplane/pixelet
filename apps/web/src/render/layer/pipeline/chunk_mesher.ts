// @pxl/web/render/layer/pipeline/chunk_mesher.ts

/** Pure 64x64 chunk -> GPU-instance buffer meshing. */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import type { LODStep } from "@/render/lod.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE, unpack_sub_cell_fraction } from "./volume.ts";
import { type MeshRun, run_to_box } from "./voxel_scan.ts";

export type BuiltChunk = {
  cnt: u32;
  /** cnt*16 floats, one column-major scale+translation matrix per instance. */
  matrices: Float32Array;
  /** cnt*3 floats, linear-space RGB. */
  rgb: Float32Array;
  /** cnt*4 floats, one terrain texture layer per supersample tap. */
  layer_taps: Float32Array;
};

export type BuiltChunkSet = { terrain: BuiltChunk; objects: BuiltChunk };

/**
 * Dev-only, cumulative since page load: how many coarse-LOD instances this
 * session has written, and how many of those were low-fill enough
 * (`fill < LOW_FILL_THRESHOLD`) that a threshold-cull policy (drop instead
 * of shrink — see chunk_stage.tsx's lod_build_stats sibling) would have
 * skipped them outright. Answers "what does always-shrink-never-drop
 * actually cost" with a real count instead of a guess. Exposed the same
 * no-console way, via chunk_stage.tsx's __PXL_RENDER_STATS__.
 */
export const coarse_instance_stats = { total: 0, low_fill: 0 };
const LOW_FILL_THRESHOLD: f64 = 0.25;

/** Dev-only, cumulative since page load: LOD1 runs seen vs. how many were occlusion-culled (see is_run_occluded). */
export const occlusion_stats = { total_runs: 0, culled: 0 };

const EMPTY_BUILT: BuiltChunk = {
  cnt: 0,
  matrices: new Float32Array(0),
  rgb: new Float32Array(0),
  layer_taps: new Float32Array(0),
};

/** sRGB [0,1] -> linear [0,1]. */
function srgb_to_linear(c: f64): f64 {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/**
 * Writes the only transform shape this mesher emits: axis-aligned scale plus
 * translation. Keeping this as the raw GPU matrix contract avoids routing a
 * pure data build through Object3D/quaternion state just to compose a matrix.
 */
function write_scale_translation(
  matrices: Float32Array,
  cnt: u32,
  sx: f64,
  sy: f64,
  sz: f64,
  tx: f64,
  ty: f64,
  tz: f64,
): void {
  const o = cnt * 16;
  // Column-major, matching the instance-matrix layout consumed by Three and
  // WebGL/WebGPU vertex attributes.
  matrices[o] = sx;
  matrices[o + 1] = 0;
  matrices[o + 2] = 0;
  matrices[o + 3] = 0;
  matrices[o + 4] = 0;
  matrices[o + 5] = sy;
  matrices[o + 6] = 0;
  matrices[o + 7] = 0;
  matrices[o + 8] = 0;
  matrices[o + 9] = 0;
  matrices[o + 10] = sz;
  matrices[o + 11] = 0;
  matrices[o + 12] = tx;
  matrices[o + 13] = ty;
  matrices[o + 14] = tz;
  matrices[o + 15] = 1;
}

/** Writes the exact box shared with chunk_raycast.ts into one instance slot. */
function write_run_instance(
  matrices: Float32Array,
  rgb: Float32Array,
  layer_taps: Float32Array,
  cnt: u32,
  cx: u32,
  cy: u32,
  run: MeshRun,
): void {
  const box = run_to_box(cx, cy, run);
  const sz_x = box.max[0] - box.min[0];
  const sz_y = box.max[1] - box.min[1];
  const sz_z = box.max[2] - box.min[2];
  write_scale_translation(
    matrices,
    cnt,
    sz_x,
    sz_y,
    sz_z,
    box.min[0] + sz_x * 0.5,
    box.min[1] + sz_y * 0.5,
    box.min[2] + sz_z * 0.5,
  );

  const ro = cnt * 3;
  rgb[ro] = srgb_to_linear(run.r / 255);
  rgb[ro + 1] = srgb_to_linear(run.g / 255);
  rgb[ro + 2] = srgb_to_linear(run.b / 255);

  const lo = cnt * 4;
  layer_taps[lo] = run.layer_idx;
  layer_taps[lo + 1] = run.layer_idx;
  layer_taps[lo + 2] = run.layer_idx;
  layer_taps[lo + 3] = run.layer_idx;
}

function column_covers_range(runs: readonly MeshRun[], z0: i32, z1: i32): boolean {
  for (const run of runs) {
    if (run.sub_sz !== SUB_CELL_NONE) continue;
    if (run.z0 <= z0 && run.z1 >= z1) return true;
  }
  return false;
}

/**
 * OBJECT-class runs never count as "covering" whatever's below them here —
 * a placed object sitting on a terrain cell doesn't form the kind of
 * continuous solid mass this occlusion check is meant to detect (a genuine
 * buried voxel, sealed on every side by more terrain/structure). The
 * camera is a near-top-down but still oblique view, not perfectly
 * vertical, so a small standalone object never actually blocks its
 * terrain cell's top face the way a real overlying layer would — treating
 * it as if it did was culling that terrain instance outright instead of
 * just letting the object visually sit in front of it, showing through as
 * a hole (bug found 2026-08-16: placing objects visibly dug into the
 * terrain beneath them).
 */
function is_topmost_run(runs: readonly MeshRun[], run: MeshRun): boolean {
  const top: i32 = run.sub_sz !== SUB_CELL_NONE ? run.z0 : run.z1;
  for (const other of runs) {
    if (other.material_class === MaterialClass.OBJECT) continue;
    const other_top: i32 = other.sub_sz !== SUB_CELL_NONE ? other.z0 : other.z1;
    if (other_top > top) return false;
  }
  return true;
}

/**
 * A run fully surrounded by opaque neighbors — all 4 horizontal neighbor
 * columns solid across its whole z0..z1 span, and something else (never an
 * OBJECT run — see is_topmost_run's own doc comment) in its own column
 * above it — can never show a face, so it costs an instance nothing will
 * ever see. Chunk-edge columns (cx/cy at 0 or CHUNK_CELL_SPAN-1) never
 * cull: the neighboring chunk's data isn't available here, and assuming
 * it's solid would be a real correctness risk (a visible hole) for a
 * cosmetic win. Sub-cell (fractional) runs never cull or get culled
 * either — too thin, and exact placement is the whole point of them.
 */
function is_run_occluded(column_runs: readonly MeshRun[][], cx: u32, cy: u32, run: MeshRun): boolean {
  if (run.sub_sz !== SUB_CELL_NONE) return false;
  if (cx === 0 || cx === CHUNK_CELL_SPAN - 1 || cy === 0 || cy === CHUNK_CELL_SPAN - 1) return false;
  const own = column_runs[cy * CHUNK_CELL_SPAN + cx];
  if (is_topmost_run(own, run)) return false;
  if (!column_covers_range(column_runs[cy * CHUNK_CELL_SPAN + (cx - 1)], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[cy * CHUNK_CELL_SPAN + (cx + 1)], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[(cy - 1) * CHUNK_CELL_SPAN + cx], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[(cy + 1) * CHUNK_CELL_SPAN + cx], run.z0, run.z1)) return false;
  return true;
}

/**
 * Pure and allocation-owning: each call returns exactly-sized immutable
 * instance buffers for one 64x64 chunk and one LOD step, from a chunk's
 * `column_runs` (see `voxel_scan.ts#scan_all_column_runs`) — scanned once
 * from the live `ChunkVolume` and reusable across every LOD rebuild after
 * that, long after the volume itself is released back to its pool.
 */
export function build_chunk_instances(
  column_runs: readonly MeshRun[][],
  lod_step: LODStep,
): BuiltChunkSet {
  if (lod_step === 1) {
    let total_terrain: u32 = 0;
    let total_objects: u32 = 0;
    for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
      for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
        for (const run of column_runs[cy * CHUNK_CELL_SPAN + cx]) {
          occlusion_stats.total_runs++;
          if (is_run_occluded(column_runs, cx, cy, run)) {
            occlusion_stats.culled++;
            continue;
          }
          if (run.material_class === MaterialClass.OBJECT) total_objects++;
          else total_terrain++;
        }
      }
    }

    const t_matrices = new Float32Array(total_terrain * 16);
    const t_rgb = new Float32Array(total_terrain * 3);
    const t_layer_taps = new Float32Array(total_terrain * 4);
    let t_cnt: u32 = 0;

    const o_matrices = new Float32Array(total_objects * 16);
    const o_rgb = new Float32Array(total_objects * 3);
    const o_layer_taps = new Float32Array(total_objects * 4);
    let o_cnt: u32 = 0;

    for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
      for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
        for (const run of column_runs[cy * CHUNK_CELL_SPAN + cx]) {
          if (is_run_occluded(column_runs, cx, cy, run)) continue;
          if (run.material_class === MaterialClass.OBJECT) {
            write_run_instance(o_matrices, o_rgb, o_layer_taps, o_cnt++, cx, cy, run);
          } else {
            write_run_instance(t_matrices, t_rgb, t_layer_taps, t_cnt++, cx, cy, run);
          }
        }
      }
    }

    return {
      terrain: { cnt: t_cnt, matrices: t_matrices, rgb: t_rgb, layer_taps: t_layer_taps },
      objects: { cnt: o_cnt, matrices: o_matrices, rgb: o_rgb, layer_taps: o_layer_taps },
    };
  }

  return build_chunk_instances_coarse(column_runs, lod_step);
}

/** layer_taps: up to 4 distinct texture-array layers touching this bucket,
 * first-encountered order (NO_LAYER — objects, always — never added). */
type ZBucketAccum = { bz: i32; pr: f64; pg: f64; pb: f64; coverage: f64; layer_taps: u32[] };

/**
 * Linear scan, not a Map: a coarse cell's real Z-bucket count is small
 * (bounded by how many bands actually stack under it), and a plain array
 * avoids allocating a hash-table container per cell — `build_chunk_instances_coarse`
 * calls this up to `res^2` times per build (1024x for `lod_step=2`), so the
 * per-cell container itself, not just its entries, has to stay cheap.
 */
function find_or_create_bucket(buckets: ZBucketAccum[], bz: i32): ZBucketAccum {
  for (let i = 0; i < buckets.length; i++) {
    if (buckets[i].bz === bz) return buckets[i];
  }
  const acc: ZBucketAccum = { bz, pr: 0, pg: 0, pb: 0, coverage: 0, layer_taps: [] };
  buckets.push(acc);
  return acc;
}

/** Same small-N linear-scan reasoning as find_or_create_bucket — a bucket
 * realistically sees a handful of distinct layers, not enough to justify a
 * hash table. Caps at 4 (matches layer_taps' fixed-size vec4 slot); beyond
 * that, whichever 4 were encountered first win. */
function collect_layer_tap(acc: ZBucketAccum, layer_idx: u32): void {
  if (layer_idx === NO_LAYER || acc.layer_taps.length >= 4 || acc.layer_taps.includes(layer_idx)) return;
  acc.layer_taps.push(layer_idx);
}

/**
 * Folds one fine-column run into whichever `lod_step`-tall Z buckets it
 * overlaps. Material class is intentionally ignored here — terrain and
 * objects are summed into the same average, on purpose (a coarse cell's
 * color is whatever's really in it, air included via `coverage` staying
 * below `lod_step^3`). A sub-cell run occupies one fine voxel fractionally;
 * its weight is the volumetric fraction (linear fraction cubed), not 1.
 */
function accumulate_run(buckets: ZBucketAccum[], lod_step: LODStep, run: MeshRun): void {
  if (run.sub_sz !== SUB_CELL_NONE) {
    const frac = unpack_sub_cell_fraction(run.sub_sz);
    const w = frac * frac * frac;
    if (w <= 0) return;
    const acc = find_or_create_bucket(buckets, Math.floor(run.z0 / lod_step));
    acc.pr += run.r * w;
    acc.pg += run.g * w;
    acc.pb += run.b * w;
    acc.coverage += w;
    collect_layer_tap(acc, run.layer_idx);
    return;
  }

  const bz_lo: i32 = Math.floor(run.z0 / lod_step);
  const bz_hi: i32 = Math.floor(run.z1 / lod_step);
  for (let bz = bz_lo; bz <= bz_hi; bz++) {
    const bucket_lo = bz * lod_step;
    const bucket_hi = bucket_lo + lod_step - 1;
    const overlap = Math.min(run.z1, bucket_hi) - Math.max(run.z0, bucket_lo) + 1;
    if (overlap <= 0) continue;
    const acc = find_or_create_bucket(buckets, bz);
    acc.pr += run.r * overlap;
    acc.pg += run.g * overlap;
    acc.pb += run.b * overlap;
    acc.coverage += overlap;
    collect_layer_tap(acc, run.layer_idx);
  }
}

/**
 * Writes one averaged coarse-cell box. Fill fraction (`coverage /
 * lod_step^3`, in (0,1]) shrinks the box isotropically — X/Y/Z scale
 * together by the same cube-root factor — rather than rendering real alpha:
 * an opaque smaller box is visually "fades out" without transparency's
 * blend/sort cost. `acc.layer_taps` (up to 4 distinct layers, first
 * encountered, padded with NO_LAYER) become this box's texture taps, so the
 * texture-pack setting still shows *something* at coarse LOD instead of
 * always falling back to the flat averaged color.
 */
function write_averaged_instance(
  matrices: Float32Array,
  rgb: Float32Array,
  layer_taps: Float32Array,
  cnt: u32,
  bx: u32,
  by: u32,
  lod_step: LODStep,
  acc: ZBucketAccum,
): void {
  const max_coverage: f64 = lod_step * lod_step * lod_step;
  const fill: f64 = Math.min(1, acc.coverage / max_coverage);
  coarse_instance_stats.total++;
  if (fill < LOW_FILL_THRESHOLD) coarse_instance_stats.low_fill++;
  const span: f64 = lod_step * U_PER_CELL;
  const sz: f64 = span * Math.cbrt(fill);
  const cz: f64 = (acc.bz * lod_step + lod_step * 0.5) * U_PER_CELL;
  write_scale_translation(matrices, cnt, sz, sz, sz, (bx + 0.5) * span, (by + 0.5) * span, cz);

  const ro = cnt * 3;
  rgb[ro] = srgb_to_linear(acc.pr / acc.coverage / 255);
  rgb[ro + 1] = srgb_to_linear(acc.pg / acc.coverage / 255);
  rgb[ro + 2] = srgb_to_linear(acc.pb / acc.coverage / 255);

  // The shader always averages 4 taps at 25% each — padding unused slots
  // with NO_LAYER (falls back to the flat instance color) dilutes toward
  // flat color whenever fewer than 4 distinct layers touched this bucket,
  // which is the common case (most buckets are one material, not four).
  // Cycling the real taps into every slot instead keeps the blend 100%
  // texture-derived whenever at least one real layer exists, matching what
  // LOD1 already does (all 4 of its taps are the same one real layer_idx).
  const n = acc.layer_taps.length;
  const lo = cnt * 4;
  layer_taps[lo] = n > 0 ? acc.layer_taps[0 % n] : NO_LAYER;
  layer_taps[lo + 1] = n > 0 ? acc.layer_taps[1 % n] : NO_LAYER;
  layer_taps[lo + 2] = n > 0 ? acc.layer_taps[2 % n] : NO_LAYER;
  layer_taps[lo + 3] = n > 0 ? acc.layer_taps[3 % n] : NO_LAYER;
}

/**
 * Coarse LOD box-averages every fine column under each `lod_step`-wide XY
 * cell (isotropic — X/Y/Z all scale by the same `lod_step`, never Z alone),
 * binned into `lod_step`-tall Z buckets, terrain and objects summed into
 * one blended color per bucket. This is the mipmap/voxel-cone-tracing
 * screen-footprint-filtering idea, not nearest-neighbor: color mixing
 * across material boundaries is intentional here, and a cell that's partly
 * air renders as a proportionally smaller opaque box (see
 * write_averaged_instance) instead of popping to nothing at a hard
 * occupancy threshold. An object straddling a fixed cell boundary still
 * splits across two smaller boxes rather than one full one — an accepted
 * side effect of a fixed grid, not the old hard-threshold disappearing bug.
 *
 * No per-level pyramid is stored, but no rescanning happens either: every
 * `lod_step` is box-averaged from the same already-scanned `column_runs`
 * (see `build_chunk_instances`'s doc comment) — exhaustively, all
 * `lod_step^2` real columns per coarse cell, no subsampling — since reading
 * an already-computed `MeshRun[]` is cheap array indexing, not a fresh
 * band walk. The expensive part (`scan_all_column_runs`) is paid once per
 * chunk and reused across every LOD tier and every LOD transition after
 * that, the same amortization the old per-chunk surface cache had, without
 * that surface's single-height-per-column ceiling (see `accumulate_run`).
 */
export function build_chunk_instances_coarse(column_runs: readonly MeshRun[][], lod_step: LODStep): BuiltChunkSet {
  if (lod_step === 1) throw new Error("render: finest LOD requires per-run meshing");

  const res: u32 = CHUNK_CELL_SPAN / lod_step;
  const per_cell: ZBucketAccum[][] = new Array(res * res);
  let total: u32 = 0;

  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      const buckets: ZBucketAccum[] = [];
      for (let j: u32 = 0; j < lod_step; j++) {
        const cy = by * lod_step + j;
        for (let k: u32 = 0; k < lod_step; k++) {
          const cx = bx * lod_step + k;
          for (const run of column_runs[cy * CHUNK_CELL_SPAN + cx]) accumulate_run(buckets, lod_step, run);
        }
      }
      per_cell[by * res + bx] = buckets;
      total += buckets.length;
    }
  }

  const matrices = new Float32Array(total * 16);
  const rgb = new Float32Array(total * 3);
  const layer_taps = new Float32Array(total * 4);
  let cnt: u32 = 0;

  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      for (const acc of per_cell[by * res + bx]) {
        write_averaged_instance(matrices, rgb, layer_taps, cnt++, bx, by, lod_step, acc);
      }
    }
  }

  return { terrain: { cnt, matrices, rgb, layer_taps }, objects: EMPTY_BUILT };
}
