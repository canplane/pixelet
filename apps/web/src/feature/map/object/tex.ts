// @pxl/web/feature/map/object/tex.ts

import { type PlaceableCube, placeableCubeId } from "@/catalog/cell/mod.ts";
import { type CubeFace, shippedTextureKey } from "@/catalog/asset/texture.ts";

export type { CubeFace };

/**
 * The shipped image a cube of this kind renders with on this face. A cube's
 * id is `"<channel>.<KEY>"`, which is how the shipped texture table is
 * keyed, so the id is the lookup.
 *
 * Side falls back to top when a kind has no side of its own — the same rule
 * the layer arrays the chunks sample are built by (crates/pxl/web/render/src/gpu/layers.rs).
 * Only the land channel has sides today; object blocks never do.
 */
export function textureKey(cube: PlaceableCube, face: CubeFace): string | null {
  const [channel, type] = placeableCubeId(cube).split(".");
  return shippedTextureKey(channel, type, face) ?? shippedTextureKey(channel, type, "top");
}
