// @pxl/client/abi/profiler.test.ts

import { assertEquals } from "@std/assert";

import { CENSUS_TAGS, FRAME } from "@/abi/profiler.spec.ts";
import { frame_at } from "./profiler.ts";

Deno.test("frame_at - reads each lane at its word", () => {
  const words = Float64Array.from({ length: FRAME.WORDS }, (_, k) => k);
  const f = frame_at(words, 0);
  assertEquals(f.budget_ms, FRAME.budget_ms);
  assertEquals(f.zoom_tiles[2], FRAME.zoom_tiles + 2);
  assertEquals(f.census.length, CENSUS_TAGS.length);
  assertEquals(f.census[1], [FRAME.census + 2, FRAME.census + 3]);
});
