// @pxl/shared/record/texture.ts

/**
 * The texture pack this build ships: which image a cube type wears on
 * each face, by bucket key.
 *
 * A pack decorates the palette; it does not define it. A cube's identity
 * and colour are the palette row's (spec/cell/color.ts), and the pack is
 * one image per type and face on top of that, so a type with no image
 * here still draws — as its colour — and a pack can be swapped without
 * touching an id. Which image is which is the file's name under
 * asset/cube: `<channel>_<type>.png` for a top, `side_<type>.png` for a
 * land side, `block_<colour>.png` for a placed block; the bytes and keys
 * come from ingested.ts.
 */

import { INGESTED, SHEET } from "./ingested.ts";

/** Which of a box's images this is. The split is the renderer's own:
 * chunk_material.ts calls a face lateral at `abs(normal.z) < 0.5`, so a
 * bottom face wears the top image just as the top face does. */
export type CubeFace = "top" | "side";

function path_of(channel: string, type_name: string, face: CubeFace): string | null {
  const type = type_name.toLowerCase();
  if (channel === "block") return face === "top" ? `cube/block/block_${type}.png` : null;
  return face === "top" ? `cube/terrain/${channel}_${type}.png` : `cube/terrain/side_${type}.png`;
}

/** The bucket key of the image a cube type wears on a face, or null when
 * the pack has none for it — the caller decides whether a side falls back
 * to the top or the type to its colour. */
export function shipped_texture_key(channel: string, type_name: string, face: CubeFace): string | null {
  const path = path_of(channel, type_name, face);
  return path ? INGESTED[path]?.key ?? null : null;
}

/** The pack as one image, for the layer array: the sheet's key and
 * geometry, and where a type's face sits on it. */
export const SHIPPED_SHEET: { key: string; cell: number; columns: number } = SHEET;

const TILE_INDEX = new Map<string, number>(SHEET.tiles.map((path, n) => [path, n]));

/** The tile of the sheet a cube type wears on a face — its pixel origin on
 * the sheet — or null when the pack has no image for it. */
export function shipped_texture_tile(channel: string, type_name: string, face: CubeFace): { x: number; y: number } | null {
  const path = path_of(channel, type_name, face);
  const n = path ? TILE_INDEX.get(path) : undefined;
  if (n === undefined) return null;
  return { x: (n % SHEET.columns) * SHEET.cell, y: Math.floor(n / SHEET.columns) * SHEET.cell };
}
