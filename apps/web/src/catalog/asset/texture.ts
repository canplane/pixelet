// @pxl/web/catalog/asset/texture.ts

/**
 * The texture pack this build ships: which image a cube type wears on
 * each face, by bucket key.
 *
 * A pack decorates the cube catalog; it does not define it. A cube's identity
 * and colour are spec/cell's (catalog/cell/color.spec.ts), and the pack is
 * one image per type and face on top of that, so a type with no image
 * here still draws — as its colour — and a pack can be swapped without
 * touching an id. Which image is which is the file's name under
 * asset/cube: `<channel>_<type>.png` for a top, `side_<type>.png` for a
 * land side, `block_<colour>.png` for a placed block; the bytes and keys
 * come from ingested.spec.ts.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { INGESTED, SHEET } from "./ingested.spec.ts";

/** Which of a box's images this is. The split is the renderer's own:
 * chunk.wgsl calls a face lateral at `abs(normal.z) < 0.5`, so a
 * bottom face wears the top image just as the top face does. */
export type CubeFace = "top" | "side";

function pathOf(channel: string, type_name: string, face: CubeFace): string | null {
  const type = type_name.toLowerCase();
  if (channel === "block") return face === "top" ? `cube/block/block_${type}.png` : null;
  return face === "top" ? `cube/terrain/${channel}_${type}.png` : `cube/terrain/side_${type}.png`;
}

/** The bucket key of the image a cube type wears on a face, or null when
 * the pack has none for it — the caller decides whether a side falls back
 * to the top or the type to its colour. */
export function shippedTextureKey(channel: string, type_name: string, face: CubeFace): string | null {
  const path = pathOf(channel, type_name, face);
  return path ? INGESTED[path]?.key ?? null : null;
}

/** The pack as one image, for the layer array: the sheet's key and
 * geometry, and where a type's face sits on it. */
export const SHIPPED_SHEET: { key: string; cell: u32; columns: u32 } = SHEET;

const TILE_INDEX = new Map<string, number>(SHEET.tiles.map((path, n) => [path, n]));

/** The tile of the sheet a cube type wears on a face — its pixel origin on
 * the sheet — or null when the pack has no image for it. */
export function shippedTextureTile(channel: string, type_name: string, face: CubeFace): { x: u32; y: u32 } | null {
  const path = pathOf(channel, type_name, face);
  const n = path ? TILE_INDEX.get(path) : undefined;
  if (n === undefined) return null;
  return { x: (n % SHEET.columns) * SHEET.cell, y: Math.floor(n / SHEET.columns) * SHEET.cell };
}
