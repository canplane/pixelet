// @pxl/web/module/world/lod.ts

/**
 * The LOD criterion as the glue's side of it: the ladder the chunks climb
 * is the module's (crates/pxl-core/src/world/lod.rs, the same numbers), and
 * what stays here is what keeps step with it outside the module — the
 * badge tiers' shift over the cube tiers, the billboards' fade range, the
 * badge occlusion's cell per distance, decor's target cell — and
 * `lod_step_at` for the badge ladder, which climbs the same rungs with the
 * same hysteresis. The projection is told to both each frame.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

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
 * factor the badges and billboard/distance_fade.ts size their own screen
 * measurements with. */
function px_per_unit(viewport_h: f64, fov_deg: f64): f64 {
  return viewport_h / (2 * Math.tan((fov_deg * Math.PI) / 360));
}

/** Six pixels at the reference. Exported for whoever applies the same
 * criterion to something other than a cell — decor, by its extent. */
export const TARGET_CELL_PX: f64 = px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0;

/** The handoff distances, finest first. Derived, so they move with the
 * projection; each is twice the one before, because each tier's cell is. */
let LOD_THRESHOLDS: f64[] = [];

function recompute_thresholds(viewport_h: f64, fov_deg: f64): void {
  const k = px_per_unit(viewport_h, fov_deg) / TARGET_CELL_PX;
  LOD_THRESHOLDS = LOD_STEP_BY_LEVEL.slice(0, -1).map((step) => step * U_PER_CELL * k);
  threshold_0 = LOD_THRESHOLDS[0];
  LOD_CELL_PER_DISTANCE.value = 2 / LOD_THRESHOLDS[0];
}

/**
 * The ladder's own scale, live with the projection: the first handoff
 * distance in world units — 192u at the reference viewport and field of
 * view, more on a taller screen, less on a shorter one. Everything that
 * should keep step with the terrain's tiers is a multiple of it, and the
 * multiples live here beside it so the proportions are read in one place.
 */
let threshold_0: f64 = REFERENCE_THRESHOLD_0;

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
const BADGE_CELLS_PER_CUBE: f64 = 4;
/** The same, as a shift on the lattice: a badge node sits this many tiers
 * above the cube tier at its range. */
export const BADGE_SHAMT_OVER_CUBE: u32 = Math.log2(BADGE_CELLS_PER_CUBE);
if (!Number.isInteger(BADGE_SHAMT_OVER_CUBE)) throw new Error("render: BADGE_CELLS_PER_CUBE must be a power of two");

/**
 * Where a world-anchored billboard starts to fade and where it is gone, in
 * first-handoff units (billboard/distance_fade.ts): full strength inside
 * 5/4 of the first handoff, gone by 10/3 of it — 240u and 640u at the
 * reference. Twice the 120u and 320u the fade was first tuned at, which
 * read as too near once the badges gathered by the cube ladder
 * (2026-09-12); and moving with the projection now rather than staying
 * fixed while the tiers under them moved.
 */
const FADE_START_TIERS: f64 = 240 / REFERENCE_THRESHOLD_0;
const FADE_END_TIERS: f64 = 640 / REFERENCE_THRESHOLD_0;

export function fade_start_u(): f64 {
  return FADE_START_TIERS * threshold_0;
}
export function fade_end_u(): f64 {
  return FADE_END_TIERS * threshold_0;
}

let projection_h: f64 = REFERENCE_VIEWPORT_H;
let projection_fov: f64 = REFERENCE_FOV_DEG;
recompute_thresholds(projection_h, projection_fov);

/**
 * Tells the ladder what it is projecting onto. Called once a frame by the
 * tick (core_tick.tsx), and a no-op while neither value moves.
 *
 * Viewport height in CSS pixels, not device pixels: at dpr 2 the same cell
 * covers twice the device pixels, and following that would double the
 * geometry a retina screen builds. Supersampling is what dpr is spent on
 * here (issue/20260911-taa-tried-and-closed.md), while which tier to
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

// Internal sampling uses 1/2/4/8/16/32/64 L1 blocks inside one chunk.
//
// Per-threshold hysteresis margin, not one flat factor for every threshold
// at once (the previous version): LOD_MARGIN's ~29% dead zone is far wider
// than BLEND_BAND_FRACTION's 5% band, and for a blend-eligible threshold
// that mismatch is a real bug, not just a wider safety margin — while
// zooming in, the blend band turns off (the chunk exits it) once dist
// crosses the band's own inner edge (t*(1-BLEND_BAND_FRACTION)), but the
// flat-factor version didn't actually commit lod_step to the finer tier
// until dist reached the much farther t/LOD_MARGIN. In the gap between
// those two points, the chunk had already looked like it switched to fine
// (the blend band's own per-chunk u_blend_r comparison shows the finer draw
// for almost any hash value near the band's low edge, since coarse_fraction
// there is near 0) and then visibly snapped back to full-coarse the instant
// the band turned off and the guide fell back to whatever lod_step
// actually was (still coarse) — a real "un-does itself" pop during smooth,
// one-directional zooming, not requiring any reversal in the input. Matching
// the blend-eligible margin to the band's own inner edge (1-BLEND_BAND_FRACTION)
// makes both systems commit to the finer tier at the exact same dist, so
// there's no gap left to fall into. Non-blend-eligible thresholds (LOD2+)
// have no band to stay consistent with, so they keep the original, wider
// LOD_MARGIN-based cushion — a hard pop there is already the least
// noticeable case (see BLEND_ELIGIBLE_THRESHOLDS's own comment).
/**
 * The tier a thing at `dist` shows, given the tier it shows now — the one
 * criterion every lattice consumer climbs, cubes and badges alike, so they
 * hand off at the same distances with the same hysteresis and move
 * together on screen.
 */
export function lod_step_at(dist: f64, current_lod_step: LodStep): LodStep {
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

/** A step's place on the ladder, which is also its shift: step = 1 << level. */
export function get_lod_level(lod_step: LodStep): LodLevel {
  if (lod_step === 1) return 0;
  if (lod_step === 2) return 1;
  if (lod_step === 4) return 2;
  if (lod_step === 8) return 3;
  if (lod_step === 16) return 4;
  if (lod_step === 32) return 5;
  return 6;
}
