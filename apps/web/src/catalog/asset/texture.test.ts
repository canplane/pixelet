// @pxl/web/catalog/asset/texture.test.ts

import { assert } from "@std/assert";

import { shippedTopTextureKey } from "./texture.ts";

Deno.test("shipped textures - a land, a color and a water top", () => {
  assert(shippedTopTextureKey("ground.land")?.startsWith("asset/"));
  assert(shippedTopTextureKey("object.white")?.startsWith("asset/"));
  assert(shippedTopTextureKey("ground.water")?.startsWith("asset/"));
});
