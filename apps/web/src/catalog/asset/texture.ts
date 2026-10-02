// @pxl/web/catalog/asset/texture.ts

/**
 * The texture pack: which image a cube type wears on top, by bucket key. A
 * type without one draws as its color.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { INGESTED, SHEET } from "./ingested.spec.ts";

const TOP_TILE = new Map<string, u32>(SHEET.faces.filter(([, , face]) => face === "top").map(([channel, type, , n]) => [`${channel}/${type}`, n]));

export function shippedTopTextureKey(channel: string, typeName: string): string | null {
  const n = TOP_TILE.get(`${channel}/${typeName.toLowerCase()}`);
  return n === undefined ? null : INGESTED[SHEET.tiles[n]]?.key ?? null;
}
