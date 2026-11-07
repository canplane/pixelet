// @pxl/client/ui/catalog/asset/mod.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { allCubes, cubeHex, cubeLabel } from "./mod.ts";

test("ALL_CUBES - every cube is listed once and has a color and a name", () => {
  const seen = new Set<number>();
  for (const cube of allCubes) {
    assert.ok(!seen.has(cube), `a cube listed twice: ${cube}`);
    seen.add(cube);
    assert.ok(cubeHex(cube) !== null, `no color for ${cube}`);
    assert.ok(cubeLabel(cube) !== "", `no name for ${cube}`);
  }
});
