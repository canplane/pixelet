// @pxl/web/render/lod.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

import { get_instance_cnt, get_lod_guide, get_lod_pair_visibility, set_lod_projection } from "./lod.ts";

// These are explicit world-space thresholds. They must not move when L2's
// ownership/remesh size changes.
const THRESHOLD_0 = 192;
const THRESHOLD_1 = 384;
const THRESHOLD_2 = 768; // LOD2<->LOD3 handoff
const THRESHOLD_3 = 1536; // LOD3<->LOD4 handoff
const BAND_0 = THRESHOLD_0 * 0.05; // BLEND_BAND_FRACTION
const BAND_1 = THRESHOLD_1 * 0.05;
const BAND_2 = THRESHOLD_2 * 0.05;

Deno.test("get_instance_cnt - finest step covers every cell in the chunk", () => {
  assertEquals(get_instance_cnt(1), CHUNK_CELL_SPAN * CHUNK_CELL_SPAN);
});

Deno.test("get_instance_cnt - doubling the step quarters the instance count", () => {
  const finer = get_instance_cnt(2);
  const coarser = get_instance_cnt(4);
  assertEquals(finer, coarser * 4);
});

Deno.test("get_instance_cnt - coarsest step 64 collapses one chunk to one proxy", () => {
  assertEquals(get_instance_cnt(64), 1);
});

Deno.test("get_lod_guide - the ladder follows the projection: twice the viewport height, twice the handoff distance", () => {
  // The default is the reference projection, where the first handoff is 192u.
  assertEquals(get_lod_guide(300, 1).lod_step, 2);
  try {
    set_lod_projection(2 * 1080, 50);
    assertEquals(get_lod_guide(300, 1).lod_step, 1);
  } finally {
    set_lod_projection(1080, 50);
  }
  assertEquals(get_lod_guide(300, 1).lod_step, 2);
});

Deno.test("get_lod_guide - close to the camera and far from any threshold band stays at the finest step, unblended", () => {
  const guide = get_lod_guide(10, 1);
  assertEquals(guide.lod_step, 1);
  assertEquals(guide.lod_level, 0);
  assertEquals(guide.blend_active, false);
});

Deno.test("get_lod_guide - very large distance settles at the coarsest step, unblended", () => {
  const guide = get_lod_guide(1_000_000, 64);
  assertEquals(guide.lod_step, 64);
  assertEquals(guide.lod_level, 6);
  assertEquals(guide.blend_active, false);
});

Deno.test("get_lod_guide - current fog range never becomes coarser than step 8", () => {
  const guide = get_lod_guide(900, 64);
  assertEquals(guide.lod_step, 8);
  assertEquals(guide.lod_level, 3);
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
  // Bands are [182.4, 201.6] and [364.8, 403.2] (see THRESHOLD_*/BAND_*
  // above) — 283 sits well clear of both.
  const guide = get_lod_guide(283, 2);
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

Deno.test("get_lod_guide - the last visible handoff blends before the fog edge", () => {
  const guide = get_lod_guide(THRESHOLD_2 - BAND_2, 4);
  assertEquals(guide.blend_active, true);
  assertEquals(guide.blend_finer_step, 4);
  assertEquals(guide.blend_coarser_step, 8);
  assertAlmostEquals(guide.coarse_fraction, 0, 1e-9);
});

Deno.test("get_lod_guide - thresholds past the visible range remain unblended", () => {
  const guide = get_lod_guide(THRESHOLD_3 - 6, 1);
  assertEquals(guide.blend_active, false);
  assertEquals(guide.lod_step, 8);
});

Deno.test("LOD pair visibility - finer and coarser form an exact partition", () => {
  for (const r of [0, 0.25, 0.5, 0.999]) {
    for (const t of [0, 0.25, 0.5, 1]) {
      const pair = get_lod_pair_visibility(r, t);
      assertEquals(Number(pair.finer) + Number(pair.coarser), 1);
    }
  }
});
