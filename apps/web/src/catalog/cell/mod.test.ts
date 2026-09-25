// @pxl/web/catalog/cell/mod.test.ts

import { assert, assertEquals } from "@std/assert";

import { cube_types, GroundChannelKey } from "./cube_type.spec.ts";
import { decor_kinds, decorName, placeable_cubes, placeableCubeHex, placeableCubeId } from "./mod.ts";
import { decor_types } from "./decor.spec.ts";

Deno.test("placeable_cubes - every cube is spelled once and has a color to swatch", () => {
  const seen = new Set<string>();
  for (const cube of placeable_cubes) {
    const id = placeableCubeId(cube);
    assert(!seen.has(id), `a cube spelled twice: ${id}`);
    seen.add(id);
    assert(placeableCubeHex(cube) !== null, `no color for ${id}`);
  }
  // Every flat color, under the block channel like any other, and a
  // terrain-styled cube of every ground channel.
  for (const name of Object.keys(cube_types.block)) assert(seen.has(`block.${name}`), name);
  for (const id of ["land.green_deep", "water.river", "structure.building", "transport.road_default"]) assert(seen.has(id), id);
});

Deno.test("placeableCubeId - a cube is spelled by its channel's name and its type's", () => {
  assertEquals(placeableCubeId({ channel: GroundChannelKey.LAND, type: cube_types.land.green_deep }), "land.green_deep");
  assertEquals(placeableCubeId({ channel: null, type: cube_types.block.white }), "block.white");
});

Deno.test("decor_kinds - every kind by number, named back as decor.json names it", () => {
  assertEquals(decor_kinds.map(decorName).sort(), Object.keys(decor_types).sort());
  for (const kind of decor_kinds) assertEquals(decor_types[decorName(kind)], kind);
});
