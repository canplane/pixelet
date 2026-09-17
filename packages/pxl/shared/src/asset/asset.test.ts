// @pxl/shared/asset/asset.test.ts

import { assert, assertEquals } from "@std/assert";

import { SHIPPED_SHEET, shipped_texture_key, shipped_texture_tile } from "./texture.ts";

Deno.test("shipped textures - a land top and side, a block top, and no block side", () => {
  assert(shipped_texture_key("land", "BARE", "top")?.startsWith("asset/"));
  assert(shipped_texture_key("land", "BARE", "side")?.startsWith("asset/"));
  assert(shipped_texture_key("block", "white", "top")?.startsWith("asset/"));
  assertEquals(shipped_texture_key("block", "white", "side"), null);
  assertEquals(shipped_texture_key("water", "RIVER", "side"), null);
  // The sheet holds every tile the keys name, each at a cell-aligned origin.
  assert(SHIPPED_SHEET.key.startsWith("asset/"));
  const tile = shipped_texture_tile("land", "BARE", "top")!;
  assertEquals(tile.x % SHIPPED_SHEET.cell, 0);
  assertEquals(tile.y % SHIPPED_SHEET.cell, 0);
  assertEquals(shipped_texture_tile("block", "white", "side"), null);
});
