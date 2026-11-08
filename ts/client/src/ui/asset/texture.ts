// @pxl/client/ui/asset/texture.ts

/**
 * Which image of the texture pack a cube wears on its top face, named by its
 * key in the bucket. A cube without one is drawn in its color.
 */

import { ASSET_FILES, PACK } from "@/service/origin/ingest.spec.ts";
import type { Cube } from "@/ui/asset/catalog.ts";

export function shippedTopTextureKey(cube: Cube): string | null {
  const tile = PACK.top[cube];
  return tile === undefined
    ? null
    : (ASSET_FILES[PACK.tiles[tile] - 1]?.key ?? null);
}
