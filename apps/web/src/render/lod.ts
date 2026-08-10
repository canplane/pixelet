// @pxl/web/render/lod.ts

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { L2_SHAMT } from "@pxl/shared/spec/space/indexer.ts";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

const CHUNK_U: u32 = 1 << L2_SHAMT;

export const LOD_STEP_BY_LEVEL = [1, 2, 4, 8, 16, 32] as const;

export type LODLevel = 0 | 1 | 2 | 3 | 4 | 5;
export type LODStep = (typeof LOD_STEP_BY_LEVEL)[LODLevel];

const LOD_MARGIN: f64 = 1.4;

const LOD_DIST_BY_LEVEL = {
  // Every finite threshold below is 2x an earlier round of chunk-width
  // multipliers (3/6/24/48/96 -> 6/12/48/96/192) — a second, independent
  // widening on top of the LOD2-4 one two paragraphs down, this time across
  // every level including LOD0/1, requested directly because LOD was
  // switching too readily/abruptly during ordinary movement. Doubling every
  // threshold doubles how far you have to travel (in any direction) before
  // a level hands off, i.e. each level now covers roughly 4x the ground
  // area (radius x2 -> area x4) before the next one takes over — same
  // ladder of steps, just stretched so a normal pan/zoom crosses far fewer
  // of them.
  LOD0: CHUNK_U * 6,
  LOD1: CHUNK_U * 12,
  // LOD2-4 thresholds were already pushed to 2x their original chunk-width
  // multiplier once before (12/24/48 -> 24/48/96) to match CELL_SHAMT going
  // -2 -> -1 (U_PER_CELL doubled): only LOD0/LOD1 are dither-blended (see
  // BLEND_ELIGIBLE_THRESHOLDS below), so the LOD2/3/4 handoffs are still
  // hard pops, and a hard pop's on-screen footprint scales with the
  // physical cell size the same way everything else here does — doubling
  // the switch distance kept that footprint (and thus how noticeable the
  // pop is) roughly where it was before the cell size changed, instead of
  // popping at the same distance as before but twice as large. Deliberately
  // NOT widening BLEND_ELIGIBLE_THRESHOLDS to cover these instead — that
  // was already tried (see its own comment) and reverted for being
  // noisier, and a larger base cell size only makes that worse, not better.
  LOD2: CHUNK_U * 48,
  LOD3: CHUNK_U * 96,
  LOD4: CHUNK_U * 192,
  LOD5: Infinity,
} as const;

/** The 5 finite distances where one LOD level hands off to the next. */
const LOD_THRESHOLDS: readonly f64[] = [
  LOD_DIST_BY_LEVEL.LOD0,
  LOD_DIST_BY_LEVEL.LOD1,
  LOD_DIST_BY_LEVEL.LOD2,
  LOD_DIST_BY_LEVEL.LOD3,
  LOD_DIST_BY_LEVEL.LOD4,
];

/**
 * Half-width (fraction of the threshold's own distance) of the band where
 * two LOD steps dither-blend. At 0.2, a wide pulled-back/top-down view (a
 * huge range of per-chunk distances at once) put over half of all loaded
 * chunks inside some band simultaneously (438/800 chunk-mesh slots, walked
 * live) — the overlapping dither read as uniform speckle, arguably worse
 * than the hard pop it was built to hide. 0.05 keeps each band narrow
 * enough that only chunks genuinely near one threshold ever blend.
 */
const BLEND_BAND_FRACTION: f64 = 0.05;

/**
 * Only the first N thresholds get dither-blended. Coarser LOD rings cover
 * far more chunks by count (ring area grows with distance), so blending
 * them too meant a wide/top-down view had a quarter or more of all loaded
 * chunks dithering at once (confirmed live: 26%, still visibly noisy even
 * with the narrower band above). Those far rings are also where a cell is
 * smallest on screen — the least noticeable case for a hard pop — so this
 * narrows blending to where the jump is both large and visible.
 */
const BLEND_ELIGIBLE_THRESHOLDS: u32 = 2;

function clamp01(x: f64): f64 {
  return x < 0 ? 0 : x > 1 ? 1 : x;
}

// internal sampling uses step values 1/2/4/8/16/32, while external naming uses LOD0..LOD5.
//
// Per-threshold hysteresis margin, not one flat factor for every threshold
// at once (the previous version): LOD_MARGIN's ~29% dead zone is far wider
// than BLEND_BAND_FRACTION's 5% band, and for a blend-eligible threshold
// that mismatch is a real bug, not just a wider safety margin — while
// zooming in, get_lod_guide's band turns off (the chunk exits it) once dist
// crosses the band's own inner edge (t*(1-BLEND_BAND_FRACTION)), but the
// flat-factor version didn't actually commit lod_step to the finer tier
// until dist reached the much farther t/LOD_MARGIN. In the gap between
// those two points, the chunk had already looked like it switched to fine
// (the blend band's own per-chunk u_blend_r comparison shows the finer draw
// for almost any hash value near the band's low edge, since coarse_fraction
// there is near 0) and then visibly snapped back to full-coarse the instant
// the band turned off and get_lod_guide fell back to whatever lod_step
// actually was (still coarse) — a real "un-does itself" pop during smooth,
// one-directional zooming, not requiring any reversal in the input. Matching
// the blend-eligible margin to the band's own inner edge (1-BLEND_BAND_FRACTION)
// makes both systems commit to the finer tier at the exact same dist, so
// there's no gap left to fall into. Non-blend-eligible thresholds (LOD2+)
// have no band to stay consistent with, so they keep the original, wider
// LOD_MARGIN-based cushion — a hard pop there is already the least
// noticeable case (see BLEND_ELIGIBLE_THRESHOLDS's own comment).
function calculate_lod_step(dist: f64, current_lod_step: LODStep): LODStep {
  for (let i = 0; i < LOD_THRESHOLDS.length; i++) {
    const raw_t: f64 = LOD_THRESHOLDS[i];
    const finer_step: LODStep = LOD_STEP_BY_LEVEL[i];

    // hysteresis only ever resists *becoming finer* (shrinking the
    // threshold once already coarser than this tier) — there's no
    // symmetric cushion against becoming coarser, same asymmetry the
    // original had; see this function's own doc comment for why that's
    // fine on the coarsening side (it never opens a band/hysteresis gap
    // the way the finer-side mismatch did).
    const already_coarser = current_lod_step > finer_step;
    const margin: f64 = i < BLEND_ELIGIBLE_THRESHOLDS ? (1 - BLEND_BAND_FRACTION) : (1 / LOD_MARGIN);
    const thres: f64 = already_coarser ? raw_t * margin : raw_t;

    if (dist < thres) return finer_step;
  }
  return LOD_STEP_BY_LEVEL[LOD_STEP_BY_LEVEL.length - 1];
}

function get_lod_level(lod_step: LODStep): LODLevel {
  if (lod_step === 1) return 0;
  if (lod_step === 2) return 1;
  if (lod_step === 4) return 2;
  if (lod_step === 8) return 3;
  if (lod_step === 16) return 4;
  return 5;
}

export function get_instance_cnt(lod_step: LODStep): u32 {
  const side: u32 = Math.floor(CHUNK_CELL_SPAN / lod_step);
  return side * side;
}

export type LODGuide = {
  lod_level: LODLevel;
  lod_step: LODStep;
  instance_cnt: u32;

  /**
   * True inside a transition band around one of the LOD_THRESHOLDS — the
   * chunk that owns this guide should mount a *second* instancedMesh at
   * blend_other_step (chunk.tsx) alongside its normal lod_step one. Two
   * neighboring chunks that both straddle the same threshold (the exact
   * scenario that showed up as a hard-edged tile/LOD boundary — see the
   * commit this was written to fix) end up with similar coarse_fraction
   * values and thus a similar finer/coarser instance mix, turning what was a
   * single-step pop into a shared gradient across the pair.
   */
  blend_active: boolean;
  /**
   * Monotonic 0 (should render as pure blend_finer_step) -> 1 (pure
   * blend_coarser_step) position within the band. Fed to both meshes as
   * u_blend_t; each instance's own per-cell hash decides whether it's on the
   * "already switched" or "not yet switched" side of that fraction — see
   * position_core in shader.ts for why this is a clean partition (every
   * covered spot is claimed by exactly one of the two draws, never both,
   * never neither) rather than plain alpha blending. Meaningless when
   * !blend_active.
   */
  coarse_fraction: f64;
  /** The finer of the two steps this threshold separates. null unless blend_active. */
  blend_finer_step: LODStep | null;
  /** The coarser of the two steps this threshold separates. null unless blend_active. */
  blend_coarser_step: LODStep | null;
};

export function get_lod_guide(dist: f64, current_lod_step: LODStep): LODGuide {
  const lod_step: LODStep = calculate_lod_step(dist, current_lod_step);
  const lod_level: LODLevel = get_lod_level(lod_step);
  const instance_cnt: u32 = get_instance_cnt(lod_step);

  const blend_eligible_len: u32 = Math.min(LOD_THRESHOLDS.length, BLEND_ELIGIBLE_THRESHOLDS);
  for (let i = 0; i < blend_eligible_len; i++) {
    const t: f64 = LOD_THRESHOLDS[i];
    const half_band: f64 = t * BLEND_BAND_FRACTION;
    if (dist < t - half_band || dist > t + half_band) continue;

    const finer: LODStep = LOD_STEP_BY_LEVEL[i];
    const coarser: LODStep = LOD_STEP_BY_LEVEL[i + 1];
    // calculate_lod_step's hysteresis can occasionally leave lod_step
    // outside this specific {finer, coarser} pair even while `dist` sits
    // inside this threshold's band (e.g. right after a large, sudden
    // distance jump, before the per-chunk memoized current_lod_step catches
    // up next frame) — nothing sensible to blend toward in that case.
    if (lod_step !== finer && lod_step !== coarser) break;

    return {
      lod_level,
      lod_step,
      instance_cnt,
      blend_active: true,
      coarse_fraction: clamp01((dist - (t - half_band)) / (2 * half_band)),
      blend_finer_step: finer,
      blend_coarser_step: coarser,
    };
  }

  return {
    lod_level,
    lod_step,
    instance_cnt,
    blend_active: false,
    coarse_fraction: 0,
    blend_finer_step: null,
    blend_coarser_step: null,
  };
}
