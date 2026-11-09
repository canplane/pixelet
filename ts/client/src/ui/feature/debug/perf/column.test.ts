// @pxl/client/ui/feature/debug/perf/column.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { FRAME, FRAME_GROUP } from "@/abi/perf.spec.ts";
import { is_list, COLUMNS } from "@/service/core/perf.ts";
import { GROUPS, LIST_NAMES } from "./column.ts";

test("FRAME_GROUP - every column is printed, in one group", () => {
  const grouped = GROUPS.flatMap((g) => FRAME_GROUP[g] as readonly string[]);
  assert.deepStrictEqual(
    [...grouped].sort(),
    COLUMNS.map((l) => l.name as string).sort(),
  );
});

test("COLUMNS - a list column spans a word a name", () => {
  for (const { name, width } of COLUMNS) {
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
