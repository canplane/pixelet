// @pxl/web/catalog/asset/texture.ts

/**
 * The texture pack this build ships: which image a cube type wears on top,
 * by bucket key.
 *
 * A pack decorates the cube catalog; it does not define it. A cube's identity
 * and color are spec/cell's (catalog/cell/color.spec.ts), and the pack is
 * one image per type and face on top of that, so a type with no image
 * here still draws — as its color — and a pack can be swapped without
 * touching an id. Which image is which is the pack's manifest
 * (asset/cube/manifest.json), which the ingest writes into
 * ingested.spec.ts as the cube faces each tile is, with the bytes and keys.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { INGESTED, SHEET } from "./ingested.spec.ts";

/** Each cube kind's top image, by channel and type name: the face a swatch
 * shows. The sheet's side images are the renderer's alone
 * (crates/pxl/web/render/src/gpu/layers.rs), which no swatch shows. */
const TOP_TILE = new Map<string, u32>(SHEET.faces.filter(([, , face]) => face === "top").map(([channel, type, , n]) => [`${channel}/${type}`, n]));

/** The shipped image a cube kind wears on top, by its bucket key; none where
 * the pack has none for it, and it draws as its color. */
export function shippedTopTextureKey(channel: string, typeName: string): string | null {
  const n = TOP_TILE.get(`${channel}/${typeName.toLowerCase()}`);
  return n === undefined ? null : INGESTED[SHEET.tiles[n]]?.key ?? null;
}
