// @pxl/web/render/layer/pipeline/chunk_mesher.ts

/** Pure 64x64 chunk -> GPU-instance buffer meshing. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import type { LODStep } from "@/render/lod.ts";
import { type ChunkVolume, L2_BAND_CELL_SPAN, MaterialClass, NO_LAYER } from "./volume.ts";
import { alloc_chunk_surface, type ChunkSurface } from "./chunk_surface.ts";
import { merge_column_runs, type MeshRun, run_to_box } from "./voxel_scan.ts";

const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;
const COARSE_FLOOR_REAL: f64 = -128 * U_PER_CELL;

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

export type { ChunkSurface } from "./chunk_surface.ts";

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

/** One direct top-down scan; reused by every coarse LOD for this batch. */
export function build_chunk_surface(volume: ChunkVolume): ChunkSurface {
  const surface = alloc_chunk_surface();
  const { z, material, layer_idx } = surface;

  for (let column: u32 = 0; column < CHUNK_CELL_SPAN2; column++) {
    const column_base = column * L2_BAND_CELL_SPAN;
    for (let si = volume.bands.length - 1; si >= 0; si--) {
      const band = volume.bands[si];
      for (let local_z = L2_BAND_CELL_SPAN - 1; local_z >= 0; local_z--) {
        const idx = column_base + local_z;
        const packed = band.material[idx];
        if (packed === 0) continue;
        z[column] = band.band_z * L2_BAND_CELL_SPAN + local_z;
        material[column] = packed;
        layer_idx[column] = band.layer_idx[idx];
        si = -1;
        break;
      }
    }
  }
  return surface;
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

function tap_layer_idx(surface: ChunkSurface, cx: u32, cy: u32): u32 {
  if (cx >= CHUNK_CELL_SPAN || cy >= CHUNK_CELL_SPAN) return NO_LAYER;
  const idx = cy * CHUNK_CELL_SPAN + cx;
  return surface.material[idx] === 0 ? NO_LAYER : surface.layer_idx[idx];
}

/** Writes the exact box shared with chunk_raycast.ts into one instance slot. */
function write_run_instance(
  matrices: Float32Array,
  rgb: Float32Array,
  layer_taps: Float32Array,
  cnt: u32,
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  run: MeshRun,
): void {
  const box = run_to_box(volume, cx, cy, run);
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

/**
 * Pure and allocation-owning: each call returns exactly-sized immutable
 * instance buffers for one 64x64 chunk (all occupied signed-Z bands)
 * and one LOD step. Vertical material runs intentionally cross band
 * boundaries: storage ownership must not manufacture buried render faces.
 */
export function build_chunk_instances(
  volume: ChunkVolume,
  lod_step: LODStep,
  cached_surface?: ChunkSurface,
): BuiltChunkSet {
  if (lod_step === 1) {
    const per_column: MeshRun[][] = new Array(CHUNK_CELL_SPAN2);
    let total_terrain: u32 = 0;
    let total_objects: u32 = 0;
    for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
      for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
        const runs = merge_column_runs(volume, cx, cy);
        per_column[cy * CHUNK_CELL_SPAN + cx] = runs;
        for (const run of runs) {
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
        const runs = per_column[cy * CHUNK_CELL_SPAN + cx];
        for (const run of runs) {
          if (run.material_class === MaterialClass.OBJECT) {
            write_run_instance(o_matrices, o_rgb, o_layer_taps, o_cnt++, volume, cx, cy, run);
          } else {
            write_run_instance(t_matrices, t_rgb, t_layer_taps, t_cnt++, volume, cx, cy, run);
          }
        }
      }
    }

    return {
      terrain: { cnt: t_cnt, matrices: t_matrices, rgb: t_rgb, layer_taps: t_layer_taps },
      objects: { cnt: o_cnt, matrices: o_matrices, rgb: o_rgb, layer_taps: o_layer_taps },
    };
  }

  return build_chunk_instances_from_surface(cached_surface ?? build_chunk_surface(volume), lod_step);
}

/** Coarse meshing never needs the dense signed-Z composite. */
export function build_chunk_instances_from_surface(surface: ChunkSurface, lod_step: LODStep): BuiltChunkSet {
  if (lod_step === 1) throw new Error("render: finest LOD requires the dense composite");

  const res: u32 = CHUNK_CELL_SPAN / lod_step;
  const cap: u32 = res * res;
  const matrices = new Float32Array(cap * 16);
  const rgb = new Float32Array(cap * 3);
  const layer_taps = new Float32Array(cap * 4);
  let cnt: u32 = 0;

  const q1: u32 = Math.floor(lod_step / 4);
  const q3: u32 = Math.floor((lod_step * 3) / 4);

  for (let by = 0; by < res; by++) {
    for (let bx = 0; bx < res; bx++) {
      let n = 0;
      let h_sum = 0, r_sum = 0, g_sum = 0, b_sum = 0;

      for (let dy = 0; dy < lod_step; dy++) {
        const cy = by * lod_step + dy;
        for (let dx = 0; dx < lod_step; dx++) {
          const cx = bx * lod_step + dx;
          const idx = cy * CHUNK_CELL_SPAN + cx;
          const packed = surface.material[idx];
          if (packed === 0) continue;
          n++;
          h_sum += surface.z[idx] + 1;
          r_sum += (packed >>> 16) & 255;
          g_sum += (packed >>> 8) & 255;
          b_sum += packed & 255;
        }
      }
      if (n === 0) continue;

      const cx0: u32 = bx * lod_step;
      const cy0: u32 = by * lod_step;
      const top_real: f64 = (h_sum / n) * U_PER_CELL;
      if (top_real <= COARSE_FLOOR_REAL) continue;

      const visual_span: f64 = lod_step * U_PER_CELL;
      const cz_span: f64 = top_real - COARSE_FLOOR_REAL;
      write_scale_translation(
        matrices,
        cnt,
        visual_span,
        visual_span,
        cz_span,
        (bx + 0.5) * visual_span,
        (by + 0.5) * visual_span,
        (top_real + COARSE_FLOOR_REAL) * 0.5,
      );

      const ro = cnt * 3;
      rgb[ro] = srgb_to_linear(r_sum / n / 255);
      rgb[ro + 1] = srgb_to_linear(g_sum / n / 255);
      rgb[ro + 2] = srgb_to_linear(b_sum / n / 255);

      const lo = cnt * 4;
      layer_taps[lo] = tap_layer_idx(surface, cx0 + q1, cy0 + q1);
      layer_taps[lo + 1] = tap_layer_idx(surface, cx0 + q1, cy0 + q3);
      layer_taps[lo + 2] = tap_layer_idx(surface, cx0 + q3, cy0 + q1);
      layer_taps[lo + 3] = tap_layer_idx(surface, cx0 + q3, cy0 + q3);
      cnt++;
    }
  }

  return {
    terrain: {
      cnt,
      matrices: matrices.slice(0, cnt * 16),
      rgb: rgb.slice(0, cnt * 3),
      layer_taps: layer_taps.slice(0, cnt * 4),
    },
    objects: EMPTY_BUILT,
  };
}
