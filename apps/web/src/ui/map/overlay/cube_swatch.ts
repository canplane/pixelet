// @pxl/web/ui/map/overlay/cube_swatch.ts

import type { CSSProperties } from "react";
import { placeable_cube_hex, type PlaceableCube } from "@/catalog/cell/mod.ts";
import { asset_url } from "@/config/route.ts";
import { texture_key } from "@/catalog/object/tex.ts";

function placeable_cube_texture_url(cube: PlaceableCube): string | null {
  const key = texture_key(cube, "top");
  return key ? asset_url(key) : null;
}

/** Style for a cube swatch — the real texture-pack top image when one
 * exists, falling back to a flat color swatch otherwise (the same fallback
 * hover_cursor.tsx's own ghost preview already relies on for
 * object/tex.ts's own get_object_texture). Shared by drawer_modal.tsx and
 * mode_bar.tsx so their swatches never drift apart. */
export function cube_swatch_style(cube: PlaceableCube): CSSProperties {
  const texture_url = placeable_cube_texture_url(cube);
  if (texture_url) {
    return { backgroundImage: `url(${texture_url})`, backgroundSize: "cover", imageRendering: "pixelated" };
  }
  return { backgroundColor: placeable_cube_hex(cube) ?? undefined };
}
