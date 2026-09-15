// @pxl/web/module/world/object/tex.ts

import { is_terrain_style_cube, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { type CubeFace, shipped_texture_key } from "@pxl/shared/record/texture.ts";

export type { CubeFace };

/**
 * The shipped image a cube of this kind renders with on this face, flat
 * colour or terrain-styled alike. A terrain-styled pick is
 * `"<channel>.<KEY>"` and a flat colour is a `block` type, which is how the
 * shipped texture table is keyed, so the two catalogs differ only in the
 * channel.
 *
 * Side falls back to top when a kind has no side of its own — the same rule
 * the layer arrays the chunks sample are built by (crates/pxl/core/src/render/gpu/layers.rs).
 * Only the land channel has sides today; object blocks never do.
 */
export function texture_key(id: PlaceableCubeId, face: CubeFace): string | null {
  const [channel, type] = is_terrain_style_cube(id) ? id.split(".") : ["block", id];
  return shipped_texture_key(channel, type, face) ?? shipped_texture_key(channel, type, "top");
}
