// @pxl/client/ui/shared/asset/catalog.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { cubeHex, cubeLabel } from "./catalog.ts";
import { CUBES } from "./cube.spec.ts";
import { decor } from "./decor.spec.ts";

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
    { ...decor },
    { tree: 1024, boat: 1025, car: 1026, train: 1027 },
  );
});
