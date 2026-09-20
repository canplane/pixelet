// @pxl/web/catalog/cell/mod.test.ts

import { assertEquals, assertNotEquals, assertThrows } from "@std/assert";

import { cube_types } from "./cube.ts";
import {
  cube_ids,
  cubeHex,
  cubeIdFromNum,
  cubeNumId,
  GroundChannelKey,
  objectValueIsAir,
  objectValueIsPresent,
  placeable_cubes,
  placeableCubeHex,
  placeableCubeId,
  placeableCubeNumId,
  placeableCubeOf,
  terrainStyleCubeFor,
  terrainStyleCubeSource,
} from "./mod.ts";

Deno.test("cube_types.common.void - is zero, sparse-tree absence", () => {
  assertEquals(cube_types.common.void, 0);
  assertEquals(objectValueIsPresent(cube_types.common.void), false);
});

Deno.test("objectValueIsPresent - anything but VOID is present", () => {
  assertEquals(objectValueIsPresent(cube_types.common.air), true);
  assertEquals(objectValueIsPresent(cube_types.block.white), true);
});

Deno.test("cube_types.common.air/UNKNOWN - the two reserved sentinels", () => {
  assertEquals(cube_types.common.air, 0x80);
  assertEquals(cube_types.common.unknown, 0xff);
  assertEquals(objectValueIsAir(cube_types.common.air), true);
  assertEquals(objectValueIsAir(cube_types.block.white), false);
});

Deno.test("cubeNumId/cubeHex/cubeIdFromNum - round-trip every flat color", () => {
  for (const id of cube_ids) {
    const num_id = cubeNumId(id);
    assertEquals(cubeIdFromNum(num_id), id);
    assertNotEquals(cubeHex(id), null);
  }
});

Deno.test("cubeIdFromNum - throws for an unregistered numeric id", () => {
  assertThrows(() => cubeIdFromNum(0));
});

Deno.test("cube_ids - no two entries collide on the same numeric id", () => {
  const seen = new Set<number>();
  for (const id of cube_ids) {
    const num_id = cubeNumId(id);
    assertEquals(seen.has(num_id), false, `duplicate numeric id ${num_id} (${id})`);
    seen.add(num_id);
  }
});

Deno.test("placeable_cube_* - round-trips a terrain-styled cube derived from cube_types' own land/water/structure/transport channels", () => {
  const cube = terrainStyleCubeFor(GroundChannelKey.LAND, cube_types.land.green_deep)!;
  assertEquals(placeableCubeId(cube), "land.green_deep");
  const num_id = placeableCubeNumId(cube);
  assertEquals(placeableCubeOf(num_id), cube);
  assertNotEquals(placeableCubeHex(cube), null);
});

Deno.test("placeable_cube_* - a flat color still routes through cubeNumId/cubeHex, not the terrain-style path", () => {
  const white = placeableCubeOf(cube_types.block.white)!;
  assertEquals(terrainStyleCubeSource(white), null);
  assertEquals(placeableCubeNumId(white), cubeNumId("white"));
  assertEquals(placeableCubeHex(white), cubeHex("white"));
});

Deno.test("placeable_cubes - every flat color and every terrain-styled entry, no duplicates", () => {
  const seen = new Set<string>();
  for (const cube of placeable_cubes) {
    const id = placeableCubeId(cube);
    assertEquals(seen.has(id), false, `duplicate placeable id ${id}`);
    seen.add(id);
  }
  // Every terrain-styled entry adds to cube_ids' own 15 flat colors, never replaces one.
  assertEquals(seen.size, placeable_cubes.length);
  assertEquals(placeable_cubes.length > cube_ids.length, true);
  // Every flat color is present, under the block channel like any other...
  for (const id of cube_ids) assertEquals(seen.has(`block.${id}`), true);
  // ...and at least one terrain-styled entry per channel is present.
  assertEquals(seen.has("land.green_deep"), true);
  assertEquals(seen.has("water.river"), true);
  assertEquals(seen.has("structure.building"), true);
  assertEquals(seen.has("transport.road_default"), true);
});

Deno.test("placeableCubeNumId - a terrain-styled cube never collides with a flat color's own global byte", () => {
  const seen = new Set<number>();
  for (const cube of placeable_cubes) {
    const num_id = placeableCubeNumId(cube);
    assertEquals(seen.has(num_id), false, `duplicate global byte ${num_id} (${placeableCubeId(cube)})`);
    seen.add(num_id);
  }
});
