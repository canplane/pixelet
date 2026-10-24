// @pxl/client/catalog/block/mod.test.ts

import { assert } from "@std/assert";

import { allCubes, cubeHex, cubeId } from "./mod.ts";

Deno.test("ALL_CUBES - every cube is spelled once and has a color to swatch", () => {
  const seen = new Set<string>();
  for (const cube of allCubes) {
    const id = cubeId(cube);
    assert(!seen.has(id), `a cube spelled twice: ${id}`);
    seen.add(id);
    assert(cubeHex(cube) !== null, `no color for ${id}`);
  }
});
