// @pxl/web/catalog/asset/texture.ts

/**
 * The texture pack this build ships: which image a cube type wears on
 * each face, by bucket key.
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

/** Which of a box's images this is. The split is the renderer's own:
 * chunk.wgsl calls a face lateral at `abs(normal.z) < 0.5`, so a
 * bottom face wears the top image just as the top face does. */
export type CubeFace = "top" | "side";

const TILE_OF_FACE = new Map<string, u32>(SHEET.faces.map(([channel, type, face, n]) => [`${channel}/${type}/${face}`, n]));

function tileOf(channel: string, type_name: string, face: CubeFace): u32 | undefined {
  return TILE_OF_FACE.get(`${channel}/${type_name.toLowerCase()}/${face}`);
}

/** The bucket key of the image a cube type wears on a face, or null when
 * the pack has none for it — the caller decides whether a side falls back
 * to the top or the type to its color. */
export function shippedTextureKey(channel: string, type_name: string, face: CubeFace): string | null {
  const n = tileOf(channel, type_name, face);
  return n === undefined ? null : INGESTED[SHEET.tiles[n]]?.key ?? null;
}

/** The pack as one image, for the layer array: the sheet's key and
 * geometry, and where a type's face sits on it. */
export const SHIPPED_SHEET: { key: string; cell: u32; columns: u32 } = SHEET;

/** The tile of the sheet a cube type wears on a face — its pixel origin on
 * the sheet — or null when the pack has no image for it. */
export function shippedTextureTile(channel: string, type_name: string, face: CubeFace): { x: u32; y: u32 } | null {
  const n = tileOf(channel, type_name, face);
  if (n === undefined) return null;
  return { x: (n % SHEET.columns) * SHEET.cell, y: Math.floor(n / SHEET.columns) * SHEET.cell };
}
