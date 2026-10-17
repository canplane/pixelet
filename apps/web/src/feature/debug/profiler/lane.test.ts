// @pxl/web/feature/debug/profiler/lane.test.ts

import { assertEquals } from "@std/assert";

import { FRAME, FRAME_GROUPS } from "./profiler.spec.ts";
import { frameAt, GROUPS, isList, LANES, LIST_NAMES } from "./lane.ts";

Deno.test("FRAME_GROUPS - every lane is printed, in one group", () => {
  const grouped = GROUPS.flatMap((g) => FRAME_GROUPS[g] as readonly string[]);
  assertEquals([...grouped].sort(), LANES.map((l) => l.name as string).sort());
});

Deno.test("LANES - a list lane spans a word a name, a census entry two", () => {
  for (const { name, width } of LANES) {
    if (!isList(name)) assertEquals(width, 1, name);
    else assertEquals(width, LIST_NAMES[name].length * (name === "census" ? 2 : 1), name);
  }
  assertEquals(LANES.reduce((n, l) => n + l.width, 0), FRAME.WORDS);
});

Deno.test("frameAt - reads each lane at its word", () => {
  const words = Float64Array.from({ length: FRAME.WORDS }, (_, k) => k);
  const f = frameAt(words, 0);
  assertEquals(f.budget_ms, FRAME.budget_ms);
  assertEquals(f.zoom_tiles[2], FRAME.zoom_tiles + 2);
  assertEquals(f.census[1], [FRAME.census + 2, FRAME.census + 3]);
});
