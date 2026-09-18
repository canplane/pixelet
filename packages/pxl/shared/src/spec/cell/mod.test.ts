// @pxl/shared/spec/cell/mod.test.ts

import { assertEquals, assertNotEquals, assertThrows } from "@std/assert";

import { cube_types } from "./cube.ts";
import {
  cube_hex,
  cube_id_from_num,
  cube_ids,
  cube_num_id,
  GroundChannelKey,
  object_value_is_air,
  object_value_is_present,
  placeable_cube_hex,
  placeable_cube_id,
  placeable_cube_num_id,
  placeable_cube_of,
  placeable_cubes,
  terrain_style_cube_for,
  terrain_style_cube_source,
} from "./mod.ts";

Deno.test("cube_types.common.void - is zero, sparse-tree absence", () => {
  assertEquals(cube_types.common.void, 0);
  assertEquals(object_value_is_present(cube_types.common.void), false);
});

Deno.test("object_value_is_present - anything but VOID is present", () => {
  assertEquals(object_value_is_present(cube_types.common.air), true);
  assertEquals(object_value_is_present(cube_types.block.white), true);
});

Deno.test("cube_types.common.air/UNKNOWN - the two reserved sentinels", () => {
  assertEquals(cube_types.common.air, 0x80);
  assertEquals(cube_types.common.unknown, 0xff);
  assertEquals(object_value_is_air(cube_types.common.air), true);
  assertEquals(object_value_is_air(cube_types.block.white), false);
});

Deno.test("cube_num_id/cube_hex/cube_id_from_num - round-trip every flat color", () => {
  for (const id of cube_ids) {
    const num_id = cube_num_id(id);
    assertEquals(cube_id_from_num(num_id), id);
    assertNotEquals(cube_hex(id), null);
  }
});

Deno.test("cube_id_from_num - throws for an unregistered numeric id", () => {
  assertThrows(() => cube_id_from_num(0));
});

Deno.test("cube_ids - no two entries collide on the same numeric id", () => {
  const seen = new Set<number>();
  for (const id of cube_ids) {
    const num_id = cube_num_id(id);
    assertEquals(seen.has(num_id), false, `duplicate numeric id ${num_id} (${id})`);
    seen.add(num_id);
  }
});

Deno.test("placeable_cube_* - round-trips a terrain-styled cube derived from cube_types' own land/water/structure/transport channels", () => {
  const cube = terrain_style_cube_for(GroundChannelKey.LAND, cube_types.land.green_deep)!;
  assertEquals(placeable_cube_id(cube), "land.green_deep");
  const num_id = placeable_cube_num_id(cube);
  assertEquals(placeable_cube_of(num_id), cube);
  assertNotEquals(placeable_cube_hex(cube), null);
});

Deno.test("placeable_cube_* - a flat color still routes through cube_num_id/cube_hex, not the terrain-style path", () => {
  const white = placeable_cube_of(cube_types.block.white)!;
  assertEquals(terrain_style_cube_source(white), null);
  assertEquals(placeable_cube_num_id(white), cube_num_id("white"));
  assertEquals(placeable_cube_hex(white), cube_hex("white"));
});

Deno.test("placeable_cubes - every flat color and every terrain-styled entry, no duplicates", () => {
  const seen = new Set<string>();
  for (const cube of placeable_cubes) {
    const id = placeable_cube_id(cube);
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

Deno.test("placeable_cube_num_id - a terrain-styled cube never collides with a flat color's own global byte", () => {
  const seen = new Set<number>();
  for (const cube of placeable_cubes) {
    const num_id = placeable_cube_num_id(cube);
    assertEquals(seen.has(num_id), false, `duplicate global byte ${num_id} (${placeable_cube_id(cube)})`);
    seen.add(num_id);
  }
});
