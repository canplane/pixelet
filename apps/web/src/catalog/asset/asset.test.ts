// @pxl/web/catalog/asset/asset.test.ts

import { assert, assertEquals } from "@std/assert";

import { SHIPPED_SHEET, shippedTextureKey, shippedTextureTile } from "./texture.ts";

Deno.test("shipped textures - a land top and side, a block top, and no block side", () => {
  assert(shippedTextureKey("land", "BARE", "top")?.startsWith("asset/"));
  assert(shippedTextureKey("land", "BARE", "side")?.startsWith("asset/"));
  assert(shippedTextureKey("block", "white", "top")?.startsWith("asset/"));
  assertEquals(shippedTextureKey("block", "white", "side"), null);
  assertEquals(shippedTextureKey("water", "RIVER", "side"), null);
  // The sheet holds every tile the keys name, each at a cell-aligned origin.
  assert(SHIPPED_SHEET.key.startsWith("asset/"));
  const tile = shippedTextureTile("land", "BARE", "top")!;
  assertEquals(tile.x % SHIPPED_SHEET.cell, 0);
  assertEquals(tile.y % SHIPPED_SHEET.cell, 0);
  assertEquals(shippedTextureTile("block", "white", "side"), null);
});
