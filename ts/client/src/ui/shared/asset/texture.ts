// @pxl/client/ui/shared/asset/texture.ts

import { ASSET_FILES, PACK } from "@/service/origin/ingest.spec.ts";
import type { Cube } from "@/ui/shared/asset/catalog.ts";

export function shippedTopTextureKey(cube: Cube): string | null {
  const tile = PACK.top[cube];
  return tile === undefined
    ? null
    : (ASSET_FILES[PACK.tiles[tile] - 1]?.key ?? null);
}
