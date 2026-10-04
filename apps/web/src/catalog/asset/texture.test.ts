// @pxl/web/catalog/asset/texture.test.ts

import { assert } from "@std/assert";

import { shippedTopTextureKey } from "./texture.ts";

Deno.test("shipped textures - a land, a block and a water top", () => {
  assert(shippedTopTextureKey("land", "BARE")?.startsWith("asset/"));
  assert(shippedTopTextureKey("block", "white")?.startsWith("asset/"));
  assert(shippedTopTextureKey("water", "RIVER")?.startsWith("asset/"));
});
