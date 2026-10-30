// @pxl/client/port/profiler.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { CENSUS_TAGS, FRAME } from "@/abi/profiler.spec.ts";
import { frame_at } from "./profiler.ts";

test("frame_at - reads each lane at its word", () => {
  const words = Float64Array.from({ length: FRAME.WORDS }, (_, k) => k);
  const f = frame_at(words, 0);
  assert.deepStrictEqual(f.budget_ms, FRAME.budget_ms);
  assert.deepStrictEqual(f.zoom_tiles[2], FRAME.zoom_tiles + 2);
  assert.deepStrictEqual(f.census.length, CENSUS_TAGS.length);
  assert.deepStrictEqual(f.census[1], [FRAME.census + 2, FRAME.census + 3]);
});
