// @pxl/web/catalog/block/mod.test.ts

import { assert } from "@std/assert";

import { cube } from "./cube.spec.ts";
import { allCubes, cubeHex, cubeId } from "./mod.ts";

Deno.test("ALL_CUBES - every cube is spelled once and has a color to swatch", () => {
  const seen = new Set<string>();
  for (const cube of allCubes) {
    const id = cubeId(cube);
    assert(!seen.has(id), `a cube spelled twice: ${id}`);
    seen.add(id);
    assert(cubeHex(cube) !== null, `no color for ${id}`);
  }
  for (const name of Object.keys(cube.ground)) assert(seen.has(`ground.${name}`), name);
  assert(!seen.has("object.air"));
});
