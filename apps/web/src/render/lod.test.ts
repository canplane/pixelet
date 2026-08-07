// @pxl/web/render/lod.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

import { get_instance_cnt, get_lod_guide } from "./lod.ts";

// CHUNK_U (module-internal) = 1 << L2_SHAMT = 32, so the first two
// LOD_THRESHOLDS this file exercises are CHUNK_U*3 = 96 and CHUNK_U*6 = 192 —
// re-derived here instead of imported since lod.ts intentionally keeps
// LOD_DIST_BY_LEVEL/LOD_THRESHOLDS module-private (get_lod_guide is the only
// sanctioned way to query LOD state). If lod.ts's own constants ever change,
// these numbers need updating alongside them — that coupling is deliberate,
// it's what makes this a regression test for the threshold values, not just
// for get_lod_guide's control flow.
const THRESHOLD_0 = 96;
const THRESHOLD_1 = 192;
const BAND_0 = THRESHOLD_0 * 0.05; // BLEND_BAND_FRACTION
const BAND_1 = THRESHOLD_1 * 0.05;

Deno.test("get_instance_cnt - finest step covers every cell in the chunk", () => {
  assertEquals(get_instance_cnt(1), CHUNK_CELL_SPAN * CHUNK_CELL_SPAN);
});

Deno.test("get_instance_cnt - doubling the step quarters the instance count", () => {
  const finer = get_instance_cnt(2);
  const coarser = get_instance_cnt(4);
  assertEquals(finer, coarser * 4);
});

Deno.test("get_instance_cnt - coarsest step (32) still covers at least one instance", () => {
  const cnt = get_instance_cnt(32);
  assertEquals(cnt >= 1, true);
});

Deno.test("get_lod_guide - close to the camera and far from any threshold band stays at the finest step, unblended", () => {
  const guide = get_lod_guide(10, 1);
  assertEquals(guide.lod_step, 1);
  assertEquals(guide.lod_level, 0);
  assertEquals(guide.blend_active, false);
});

Deno.test("get_lod_guide - very large distance settles at the coarsest step, unblended", () => {
  const guide = get_lod_guide(1_000_000, 32);
  assertEquals(guide.lod_step, 32);
  assertEquals(guide.lod_level, 5);
  assertEquals(guide.blend_active, false);
});

Deno.test("get_lod_guide - at the start of the first blend band, coarse_fraction reads as pure-finer (~0)", () => {
  const guide = get_lod_guide(THRESHOLD_0 - BAND_0, 1);
  assertEquals(guide.blend_active, true);
  assertEquals(guide.blend_finer_step, 1);
  assertEquals(guide.blend_coarser_step, 2);
  assertAlmostEquals(guide.coarse_fraction, 0, 1e-9);
});

Deno.test("get_lod_guide - at the end of the first blend band, coarse_fraction reads as pure-coarser (~1)", () => {
  const guide = get_lod_guide(THRESHOLD_0 + BAND_0, 2);
  assertEquals(guide.blend_active, true);
  assertEquals(guide.blend_finer_step, 1);
  assertEquals(guide.blend_coarser_step, 2);
  assertAlmostEquals(guide.coarse_fraction, 1, 1e-9);
});

Deno.test("get_lod_guide - comfortably between the first two blend bands is unblended", () => {
  // Bands are [91.2, 100.8] and [182.4, 201.6] (see THRESHOLD_*/BAND_*
  // above) — 150 sits well clear of both.
  const guide = get_lod_guide(150, 2);
  assertEquals(guide.blend_active, false);
});

Deno.test("get_lod_guide - the second blend band (LOD1<->LOD2) also blends, same shape as the first", () => {
  // current_lod_step=1 here (not 2) so calculate_lod_step's hysteresis
  // margin doesn't kick in — with a >1 current step every threshold check
  // is scaled by 1/LOD_MARGIN, which at this exact distance would already
  // push the resolved step past this pair (see the coarser-hysteresis case
  // covered by the "end of the first blend band" test above, which
  // deliberately exercises that margin instead).
  const guide = get_lod_guide(THRESHOLD_1 - BAND_1, 1);
  assertEquals(guide.blend_active, true);
  assertEquals(guide.blend_finer_step, 2);
  assertEquals(guide.blend_coarser_step, 4);
  assertAlmostEquals(guide.coarse_fraction, 0, 1e-9);
});

Deno.test("get_lod_guide - thresholds past BLEND_ELIGIBLE_THRESHOLDS never blend, even right at the edge of one", () => {
  // LOD2<->LOD3 (t=384) is the 3rd threshold — BLEND_ELIGIBLE_THRESHOLDS=2
  // only covers the first two (LOD0<->1, LOD1<->2), so this one always pops
  // instead of dithering, by design (see lod.ts's doc comment on why: far
  // rings cover the most chunks by count, and are visually the smallest
  // per-cell anyway). 380 sits just inside the LOD2 tier (step 4), a hair
  // below the 384 threshold — the closest a "step 4" distance gets to it.
  const guide = get_lod_guide(380, 1);
  assertEquals(guide.blend_active, false);
  assertEquals(guide.lod_step, 4);
});
