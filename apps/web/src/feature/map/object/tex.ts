// @pxl/web/feature/map/object/tex.ts

import { type PlaceableCube, placeableCubeChannel, placeableCubeType } from "@/catalog/cell/mod.ts";
import { shippedTopTextureKey } from "@/catalog/asset/texture.ts";

/** The shipped image a cube of this kind wears on top. The shipped texture
 * table is keyed by the catalog's channel and type names. */
export function textureKey(cube: PlaceableCube): string | null {
  return shippedTopTextureKey(placeableCubeChannel(cube), placeableCubeType(cube));
}
