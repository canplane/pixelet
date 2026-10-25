// @pxl/client/shell/catalog/asset/texture.ts

/**
 * The texture pack: which image a cube type wears on top, by bucket key. A
 * type without one draws as its color.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { INGESTED, PACK } from "@/io/ingested.spec.ts";

/** By a cube's id, its group and type (`ground.land`). */
const TOP_TILE = new Map<string, u32>(PACK.faces.filter(([, , face]) => face === "top").map(([group, type, , n]) => [`${group}.${type}`, n]));

export function shippedTopTextureKey(cubeId: string): string | null {
  const n = TOP_TILE.get(cubeId);
  return n === undefined ? null : INGESTED[PACK.tiles[n]]?.key ?? null;
}
