// @pxl/render/lod.ts

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { L2_SHAMT } from "@pxl/spec/space/indexer.ts";

import { CHUNK_CELL_SPAN } from "@pxl/bake/terrain/constants.ts";

const CHUNK_U: u32 = 1 << L2_SHAMT;

export const LOD_STEP_BY_LEVEL = [1, 2, 4, 8, 16, 32] as const;

export type LODLevel = 0 | 1 | 2 | 3 | 4 | 5;
export type LODStep = (typeof LOD_STEP_BY_LEVEL)[LODLevel];

const LOD_MARGIN: f64 = 1.4;

const LOD_DIST_BY_LEVEL = {
  LOD0: CHUNK_U * 6,
  LOD1: CHUNK_U * 12,
  LOD2: CHUNK_U * 24,
  LOD3: CHUNK_U * 48,
  LOD4: CHUNK_U * 96,
  LOD5: Infinity,
} as const;

// Internal sampling uses step values 1/2/4/8/16/32, while external naming uses LOD0..LOD5.
function calculate_lod_step(dist: f64, current_lod_step: LODStep): LODStep {
  const get_thres = (ref_dist: f64): f64 => ref_dist * (current_lod_step > 1 ? 1 / LOD_MARGIN : 1.0);

  if (dist < get_thres(LOD_DIST_BY_LEVEL.LOD0)) return 1;
  if (dist < get_thres(LOD_DIST_BY_LEVEL.LOD1)) return 2;
  if (dist < get_thres(LOD_DIST_BY_LEVEL.LOD2)) return 4;
  if (dist < get_thres(LOD_DIST_BY_LEVEL.LOD3)) return 8;
  if (dist < get_thres(LOD_DIST_BY_LEVEL.LOD4)) return 16;
  return 32;
}

function get_lod_level(lod_step: LODStep): LODLevel {
  if (lod_step === 1) return 0;
  if (lod_step === 2) return 1;
  if (lod_step === 4) return 2;
  if (lod_step === 8) return 3;
  if (lod_step === 16) return 4;
  return 5;
}

function get_instance_cnt(lod_step: LODStep): u32 {
  const side: u32 = Math.floor(CHUNK_CELL_SPAN / lod_step);
  return side * side;
}

export type LODGuide = {
  lod_level: LODLevel;
  lod_step: LODStep;
  instance_cnt: u32;
};

export function get_lod_guide(dist: f64, current_lod_step: LODStep): LODGuide {
  const lod_step: LODStep = calculate_lod_step(dist, current_lod_step);
  const lod_level: LODLevel = get_lod_level(lod_step);
  const instance_cnt: u32 = get_instance_cnt(lod_step);
  return { lod_level, lod_step, instance_cnt };
}
