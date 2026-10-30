// @pxl/client/shell/debug/profiler/lane.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { FRAME_GROUP } from "@/abi/profiler.spec.ts";
import { is_list, LANES } from "@/port/profiler.ts";
import { GROUPS, LIST_NAMES } from "./lane.ts";

test("FRAME_GROUP - every lane is printed, in one group", () => {
  const grouped = GROUPS.flatMap((g) => FRAME_GROUP[g] as readonly string[]);
  assert.deepStrictEqual(
    [...grouped].sort(),
    LANES.map((l) => l.name as string).sort(),
  );
});

test("LANES - a list lane spans a word a name, a census entry two", () => {
  for (const { name, width } of LANES) {
    if (!is_list(name)) assert.deepStrictEqual(width, 1, name);
    else {
      assert.deepStrictEqual(
        width,
        LIST_NAMES[name].length * (name === "census" ? 2 : 1),
        name,
      );
    }
  }
});
