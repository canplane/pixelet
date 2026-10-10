// @pxl/shared/spec/cell/mod.test.ts

import { assertEquals, assertNotEquals, assertThrows } from "@std/assert";

import { cube_types } from "./cube.ts";
import {
  cube_hex,
  cube_id_from_num,
  cube_ids,
  cube_num_id,
  is_terrain_style_cube,
  object_value_is_air,
  object_value_is_present,
  placeable_cube_hex,
  placeable_cube_id_from_num,
  placeable_cube_ids,
  placeable_cube_num_id,
} from "./mod.ts";

Deno.test("cube_types.common.VOID - is zero, sparse-tree absence", () => {
  assertEquals(cube_types.common.VOID, 0);
  assertEquals(object_value_is_present(cube_types.common.VOID), false);
});

Deno.test("object_value_is_present - anything but VOID is present", () => {
  assertEquals(object_value_is_present(cube_types.common.AIR), true);
  assertEquals(object_value_is_present(cube_types.block.white), true);
});

Deno.test("cube_types.common.AIR/UNKNOWN - the two reserved sentinels", () => {
  assertEquals(cube_types.common.AIR, 0x80);
  assertEquals(cube_types.common.UNKNOWN, 0xff);
  assertEquals(object_value_is_air(cube_types.common.AIR), true);
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

Deno.test("placeable_cube_* - round-trips a terrain-styled id derived from cube_types' own land/water/structure/transport channels", () => {
  const id = "land.GREEN_DEEP" as const;
  assertEquals(is_terrain_style_cube(id), true);
  const num_id = placeable_cube_num_id(id);
  assertEquals(placeable_cube_id_from_num(num_id), id);
  assertNotEquals(placeable_cube_hex(id), null);
});

Deno.test("placeable_cube_* - a flat color id still routes through cube_num_id/cube_hex, not the terrain-style path", () => {
  assertEquals(is_terrain_style_cube("white"), false);
  assertEquals(placeable_cube_num_id("white"), cube_num_id("white"));
  assertEquals(placeable_cube_hex("white"), cube_hex("white"));
});

Deno.test("placeable_cube_ids - every flat color and every terrain-styled entry, no duplicates", () => {
  const seen = new Set<string>();
  for (const id of placeable_cube_ids) {
    assertEquals(seen.has(id), false, `duplicate placeable id ${id}`);
    seen.add(id);
  }
  // Every terrain-styled entry adds to cube_ids' own 15 flat colors, never replaces one.
  assertEquals(seen.size, placeable_cube_ids.length);
  assertEquals(placeable_cube_ids.length > cube_ids.length, true);
  // Every flat color is present...
  for (const id of cube_ids) assertEquals(seen.has(id), true);
  // ...and at least one terrain-styled entry per channel is present.
  assertEquals(seen.has("land.GREEN_DEEP"), true);
  assertEquals(seen.has("water.RIVER"), true);
  assertEquals(seen.has("structure.BUILDING"), true);
  assertEquals(seen.has("transport.ROAD_DEFAULT"), true);
});

Deno.test("placeable_cube_num_id - a terrain-styled id never collides with a flat color's own global byte", () => {
  const seen = new Set<number>();
  for (const id of placeable_cube_ids) {
    const num_id = placeable_cube_num_id(id);
    assertEquals(seen.has(num_id), false, `duplicate global byte ${num_id} (${id})`);
    seen.add(num_id);
  }
});
