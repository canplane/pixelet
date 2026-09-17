// @pxl/web/catalog/object/tex.ts

import { placeable_cube_id, type PlaceableCube } from "@pxl/shared/spec/cell/mod.ts";
import { type CubeFace, shipped_texture_key } from "@pxl/shared/asset/texture.ts";

export type { CubeFace };

/**
 * The shipped image a cube of this kind renders with on this face. A cube's
 * id is `"<channel>.<KEY>"`, which is how the shipped texture table is
 * keyed, so the id is the lookup.
 *
 * Side falls back to top when a kind has no side of its own — the same rule
 * the layer arrays the chunks sample are built by (crates/pxl/kernel/src/render/gpu/layers.rs).
 * Only the land channel has sides today; object blocks never do.
 */
export function texture_key(cube: PlaceableCube, face: CubeFace): string | null {
  const [channel, type] = placeable_cube_id(cube).split(".");
  return shipped_texture_key(channel, type, face) ?? shipped_texture_key(channel, type, "top");
}
