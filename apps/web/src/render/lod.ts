// @pxl/web/render/lod.ts

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

/** XY sampling stride in L1 blocks inside one 64x64 chunk. */
export const LOD_STEP_BY_LEVEL = [1, 2, 4, 8, 16, 32, 64] as const;

export type LodLevel = 0 | 1 | 2 | 3 | 4 | 5 | 6;
export type LodStep = (typeof LOD_STEP_BY_LEVEL)[LodLevel];

const LOD_MARGIN: f64 = 1.4;

/**
 * The criterion: a tier hands off to the next when one of its cells would
 * project to about this many pixels on screen.
 *
 * Screen-space error, the ordinary rule for chunked terrain LOD, rather than
 * a table of world distances. A table is this same rule frozen at one
 * viewport and one field of view, and it cannot say what should happen when
 * either moves — which is why the value below is not a fresh one: it is the
 * ladder that was already here (192u for the finest tier at 1080px and 50
 * degrees), restated. A change of reasoning, not a retune.
 */
const REFERENCE_VIEWPORT_H: f64 = 1080;
const REFERENCE_FOV_DEG: f64 = 50;
const REFERENCE_THRESHOLD_0: f64 = 192;

/** Pixels a world unit covers at one unit of distance — the same projection
 * factor billboard/field.ts and use_distance_fade.ts size their own screen
 * measurements with. */
function px_per_unit(viewport_h: f64, fov_deg: f64): f64 {
  return viewport_h / (2 * Math.tan((fov_deg * Math.PI) / 360));
}

const TARGET_CELL_PX: f64 = px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0;

/** The handoff distances, finest first. Derived, so they move with the
 * projection; each is twice the one before, because each tier's cell is. */
let LOD_THRESHOLDS: f64[] = [];

function recompute_thresholds(viewport_h: f64, fov_deg: f64): void {
  const k = px_per_unit(viewport_h, fov_deg) / TARGET_CELL_PX;
  LOD_THRESHOLDS = LOD_STEP_BY_LEVEL.slice(0, -1).map((step) => step * U_PER_CELL * k);
  LOD_SCALE.threshold_0 = LOD_THRESHOLDS[0];
  LOD_CELL_PER_DISTANCE.value = 2 / LOD_THRESHOLDS[0];
}

/**
 * The ladder's own scale, live with the projection: the first handoff
 * distance in world units — 192u at the reference viewport and field of
 * view, more on a taller screen, less on a shorter one. Everything that
 * should keep step with the terrain's tiers is a multiple of it, and the
 * multiples live here beside it so the proportions are read in one place.
 */
export const LOD_SCALE = { threshold_0: REFERENCE_THRESHOLD_0 };

/**
 * The largest cell drawn at a range, per unit of that range. Just past a
 * threshold the tier doubles, so a chunk at the first threshold shows
 * two-cell boxes, at twice that distance four-cell ones, and so on: the
 * cell drawn at any distance is at most this fraction of it, and half that
 * just before the next handoff. A uniform object, kept current with the
 * projection, for whoever has to tolerate a coarse cell's height — the
 * badge occlusion test.
 */
export const LOD_CELL_PER_DISTANCE = { value: 2 / REFERENCE_THRESHOLD_0 };

// ---- What keeps step with the ladder ----

/**
 * How many terrain cells a place badge's cell spans at any range: the badge
 * ladder is the cube ladder times this (poi/poi_layer.ts). It was a pixel
 * width of its own, which came out at about eight cubes and gathered places
 * plainly apart on screen (2026-09-12); four is the setting, not the
 * derivation.
 */
export const BADGE_CELLS_PER_CUBE: f64 = 4;

/**
 * Where a world-anchored billboard starts to fade and where it is gone, in
 * first-handoff units (billboard/distance_fade.ts): full strength inside
 * 5/4 of the first handoff, gone by 10/3 of it — 240u and 640u at the
 * reference. Twice the 120u and 320u the fade was first tuned at, which
 * read as too near once the badges gathered by the cube ladder
 * (2026-09-12); and moving with the projection now rather than staying
 * fixed while the tiers under them moved.
 */
export const FADE_START_TIERS: f64 = 240 / REFERENCE_THRESHOLD_0;
export const FADE_END_TIERS: f64 = 640 / REFERENCE_THRESHOLD_0;

export function fade_start_u(): f64 {
  return FADE_START_TIERS * LOD_SCALE.threshold_0;
}
export function fade_end_u(): f64 {
  return FADE_END_TIERS * LOD_SCALE.threshold_0;
}

let projection_h: f64 = REFERENCE_VIEWPORT_H;
let projection_fov: f64 = REFERENCE_FOV_DEG;
recompute_thresholds(projection_h, projection_fov);

/**
 * Tells the ladder what it is projecting onto. Called once a frame by the
 * chunk scheduler, and a no-op while neither value moves.
 *
 * Viewport height in CSS pixels, not device pixels: at dpr 2 the same cell
 * covers twice the device pixels, and following that would double the
 * geometry a retina screen builds. Supersampling is what dpr is spent on
 * here (discussions/20260911-taa-tried-and-closed.md), while which tier to
 * show is a question about how large a cell looks.
 */
export function set_lod_projection(viewport_h: f64, fov_deg: f64): void {
  if (viewport_h === projection_h && fov_deg === projection_fov) return;
  projection_h = viewport_h;
  projection_fov = fov_deg;
  recompute_thresholds(viewport_h, fov_deg);
}

/**
 * Half-width (fraction of the threshold's own distance) of the band where
 * two LOD steps dither-blend. A wide pulled-back/top-down view puts a huge
 * range of per-chunk distances on screen at once, so too wide a band lets
 * over half of all loaded chunks fall inside some band simultaneously,
 * reading as uniform speckle — arguably worse than the hard pop it was
 * built to hide. 0.05 keeps each band narrow enough that only chunks
 * genuinely near one threshold ever blend.
 */
const BLEND_BAND_FRACTION: f64 = 0.05;

/**
 * Only thresholds inside the current visible range get dither-blended. At the
 * reference projection the third handoff (768u) is the last one comfortably
 * inside render_cfg.RENDER_DIST, so including it removes both its hard pop
 * and the old 29% non-blended hysteresis gap. The handoff that lands at the
 * range's own edge, and everything past it, stay hard transitions: geometry
 * ends there and fog is already opaque, so a second mesh would be retained
 * for nothing.
 */
const BLEND_ELIGIBLE_THRESHOLDS: u32 = 3;

function clamp01(x: f64): f64 {
  return x < 0 ? 0 : x > 1 ? 1 : x;
}

/**
 * Complementary stochastic visibility used by the two meshes in one LOD
 * transition. Exactly one side is visible for every r/t pair: no overlap,
 * no hole. `r` is stable per L2 chunk while `coarse_fraction` moves through
 * the band, staggering whole-chunk transitions without relief checkerboard.
 */
export function get_lod_pair_visibility(r: f64, coarse_fraction: f64): { finer: boolean; coarser: boolean } {
  const coarser = r < coarse_fraction;
  return { finer: !coarser, coarser };
}

// Internal sampling uses 1/2/4/8/16/32/64 L1 blocks inside one chunk.
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
function calculate_lod_step(dist: f64, current_lod_step: LodStep): LodStep {
  for (let i = 0; i < LOD_THRESHOLDS.length; i++) {
    const raw_t: f64 = LOD_THRESHOLDS[i];
    const finer_step: LodStep = LOD_STEP_BY_LEVEL[i];

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

function get_lod_level(lod_step: LodStep): LodLevel {
  if (lod_step === 1) return 0;
  if (lod_step === 2) return 1;
  if (lod_step === 4) return 2;
  if (lod_step === 8) return 3;
  if (lod_step === 16) return 4;
  if (lod_step === 32) return 5;
  return 6;
}

export function get_instance_cnt(lod_step: LodStep): u32 {
  const side: u32 = Math.floor(CHUNK_CELL_SPAN / lod_step);
  return side * side;
}

export type LodGuide = {
  lod_level: LodLevel;
  lod_step: LodStep;
  instance_cnt: u32;

  /**
   * True inside a transition band around one of the LOD_THRESHOLDS — the
   * chunk that owns this guide should mount a *second* instancedMesh at
   * blend_other_step (chunk.tsx) alongside its normal lod_step one. Two
   * neighboring chunks that both straddle the same threshold end up with
   * similar coarse_fraction values and thus a similar finer/coarser
   * instance mix, turning what would otherwise be a single-step pop at a
   * hard-edged tile/LOD boundary into a shared gradient across the pair.
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
  blend_finer_step: LodStep | null;
  /** The coarser of the two steps this threshold separates. null unless blend_active. */
  blend_coarser_step: LodStep | null;
};

export function get_lod_guide(dist: f64, current_lod_step: LodStep): LodGuide {
  const lod_step: LodStep = calculate_lod_step(dist, current_lod_step);
  const lod_level: LodLevel = get_lod_level(lod_step);
  const instance_cnt: u32 = get_instance_cnt(lod_step);

  const blend_eligible_len: u32 = Math.min(LOD_THRESHOLDS.length, BLEND_ELIGIBLE_THRESHOLDS);
  for (let i = 0; i < blend_eligible_len; i++) {
    const t: f64 = LOD_THRESHOLDS[i];
    const half_band: f64 = t * BLEND_BAND_FRACTION;
    if (dist < t - half_band || dist > t + half_band) continue;

    const finer: LodStep = LOD_STEP_BY_LEVEL[i];
    const coarser: LodStep = LOD_STEP_BY_LEVEL[i + 1];
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
