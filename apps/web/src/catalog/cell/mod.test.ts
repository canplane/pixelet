// @pxl/web/catalog/cell/mod.test.ts

import { assert } from "@std/assert";

import { cube_types } from "./cube_type.spec.ts";
import { placeableCubeHex, placeableCubeId, placeableCubes } from "./mod.ts";

Deno.test("placeable_cubes - every cube is spelled once and has a color to swatch", () => {
  const seen = new Set<string>();
  for (const cube of placeableCubes) {
    const id = placeableCubeId(cube);
    assert(!seen.has(id), `a cube spelled twice: ${id}`);
    seen.add(id);
    assert(placeableCubeHex(cube) !== null, `no color for ${id}`);
  }
  for (const name of Object.keys(cube_types.ground)) assert(seen.has(`ground.${name}`), name);
  assert(!seen.has("object.air"));
});
