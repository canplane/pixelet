// @pxl/client/shell/catalog/asset/mod.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { allCubes, cubeHex, cubeId } from "./mod.ts";

test("ALL_CUBES - every cube is spelled once and has a color to swatch", () => {
  const seen = new Set<string>();
  for (const cube of allCubes) {
    const id = cubeId(cube);
    assert.ok(!seen.has(id), `a cube spelled twice: ${id}`);
    seen.add(id);
    assert.ok(cubeHex(cube) !== null, `no color for ${id}`);
  }
});
