// @pxl/client/shell/catalog/asset/texture.ts

/**
 * The texture pack: which image a cube wears on top, by bucket key. A cube
 * without one draws as its color.
 */

import { ASSET_FILES, PACK } from "@/port/shell.ts";
import type { Cube } from "@/shell/catalog/asset/mod.ts";

export function shippedTopTextureKey(cube: Cube): string | null {
  const tile = PACK.top[cube];
  return tile === undefined
    ? null
    : (ASSET_FILES[PACK.tiles[tile] - 1]?.key ?? null);
}
