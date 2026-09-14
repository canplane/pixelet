// @pxl/web/module/world/lod.test.ts

import { assertEquals } from "@std/assert";

import { BADGE_SHAMT_OVER_CUBE, get_lod_level, lod_step_at, set_lod_projection } from "@/module/world/lod.ts";

// These are explicit world-space thresholds. They must not move when L2's
// ownership/remesh size changes.
const THRESHOLD_0 = 192;
const THRESHOLD_1 = 384;
const THRESHOLD_2 = 768; // LOD2<->LOD3 handoff
const THRESHOLD_3 = 1536; // LOD3<->LOD4 handoff

Deno.test("lod_step_at - the ladder follows the projection: twice the viewport height, twice the handoff distance", () => {
  // The default is the reference projection, where the first handoff is 192u.
  assertEquals(lod_step_at(300, 1), 2);
  try {
    set_lod_projection(2 * 1080, 50);
    assertEquals(lod_step_at(300, 1), 1);
  } finally {
    set_lod_projection(1080, 50);
  }
  assertEquals(lod_step_at(300, 1), 2);
});

Deno.test("lod_step_at - the rungs are the module's: 192, 384, 768, 1536 at the reference, 64 past the last", () => {
  set_lod_projection(1080, 50);
  assertEquals(lod_step_at(THRESHOLD_0 - 1, 1), 1);
  assertEquals(lod_step_at(THRESHOLD_0 + 1, 1), 2);
  assertEquals(lod_step_at(THRESHOLD_1 + 1, 1), 4);
  assertEquals(lod_step_at(THRESHOLD_2 + 1, 1), 8);
  assertEquals(lod_step_at(THRESHOLD_3 + 1, 1), 16);
  assertEquals(lod_step_at(1_000_000, 1), 64);
});

Deno.test("lod_step_at - a badge climbs the cube ladder: same handoff, same hysteresis, the badge tier a fixed shift above", () => {
  set_lod_projection(1080, 50);
  // Just past the first handoff both go coarse together.
  const past = lod_step_at(200, 1);
  assertEquals(past, 2);
  // Back just inside it, the coarser tier holds — the hysteresis a chunk has.
  assertEquals(lod_step_at(190, past), 2);
  // Well inside, both come back.
  assertEquals(lod_step_at(150, past), 1);
  assertEquals(get_lod_level(lod_step_at(200, 1)) + BADGE_SHAMT_OVER_CUBE, 3);
});
