// @pxl/client/ui/shared/asset/catalog.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { cubeHex, cubeLabel, CUBES } from "./catalog.ts";
import {
  DECOR_BOAT,
  DECOR_CAR,
  DECOR_TRAIN,
  DECOR_TREE,
} from "./decor.spec.ts";

test("ALL_CUBES - every cube is listed once and has a color and a name", () => {
  const seen = new Set<number>();
  for (const cube of CUBES) {
    assert.ok(!seen.has(cube), `a cube listed twice: ${cube}`);
    seen.add(cube);
    assert.ok(cubeHex(cube) !== null, `no color for ${cube}`);
    assert.ok(cubeLabel(cube) !== "", `no name for ${cube}`);
  }
});

test("a decor's id is stored, so the table's order never moves", () => {
  assert.deepEqual(
    [DECOR_TREE, DECOR_BOAT, DECOR_CAR, DECOR_TRAIN],
    [1024, 1025, 1026, 1027],
  );
});
