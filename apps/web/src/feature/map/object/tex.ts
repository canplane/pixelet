// @pxl/web/feature/map/object/tex.ts

import { type PlaceableCube, placeableCubeChannel, placeableCubeType } from "@/catalog/cell/mod.ts";
import { type CubeFace, shippedTextureKey } from "@/catalog/asset/texture.ts";

export type { CubeFace };

/**
 * The shipped image a cube of this kind renders with on this face. The
 * shipped texture table is keyed by the catalog's channel and type names.
 *
 * Side falls back to top when a kind has no side of its own — the same rule
 * the layer arrays the chunks sample are built by (crates/pxl/web/render/src/gpu/layers.rs).
 * Only the land channel has sides today; object blocks never do.
 */
export function textureKey(cube: PlaceableCube, face: CubeFace): string | null {
  const channel = placeableCubeChannel(cube);
  const type = placeableCubeType(cube);
  return shippedTextureKey(channel, type, face) ?? shippedTextureKey(channel, type, "top");
}
