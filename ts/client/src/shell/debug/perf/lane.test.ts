// @pxl/client/shell/debug/perf/lane.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { FRAME, FRAME_GROUP } from "@/abi/perf.spec.ts";
import { is_list, LANES } from "@/port/perf.ts";
import { GROUPS, LIST_NAMES } from "./lane.ts";

test("FRAME_GROUP - every lane is printed, in one group", () => {
  const grouped = GROUPS.flatMap((g) => FRAME_GROUP[g] as readonly string[]);
  assert.deepStrictEqual(
    [...grouped].sort(),
    LANES.map((l) => l.name as string).sort(),
  );
});

test("LANES - a list lane spans a word a name", () => {
  for (const { name, width } of LANES) {
    const names = is_list(name) ? LIST_NAMES[name].length : 1;
    assert.deepStrictEqual(width, names, name);
  }
});

test("FRAME_GROUP - the rays group lies in FRAME in its order", () => {
  const first = FRAME[FRAME_GROUP.rays[0]];
  FRAME_GROUP.rays.forEach((name, k) =>
    assert.deepStrictEqual(FRAME[name], first + k, name),
  );
});
